use std::{
    fs,
    io::{Cursor, Write},
    path::Path,
    process::{Command, Output},
};
use zip::{ZipWriter, write::SimpleFileOptions};

fn zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        writer
            .start_file(*name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}
fn epub(version: u8, cover: bool) -> Vec<u8> {
    let metadata = if version == 2 && cover {
        "<meta name=\"cover\" content=\"art\"/>"
    } else {
        ""
    };
    let properties = if version == 3 && cover {
        " properties=\"cover-image\""
    } else {
        ""
    };
    let opf = format!(
        r#"<package xmlns="http://www.idpf.org/2007/opf" version="{version}.0" unique-identifier="id"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="id">test</dc:identifier>{metadata}</metadata><manifest><item id="art" href="images/art.png" media-type="image/png"{properties}/></manifest><spine/></package>"#
    );
    zip(&[("mimetype", b"application/epub+zip"), ("META-INF/container.xml", br#"<container xmlns="urn:oasis:names:tc:opendocument:xmlns:container" version="1.0"><rootfiles><rootfile full-path="OPS/book.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#), ("OPS/book.opf", opf.as_bytes()), ("OPS/images/art.png", b"original cover bytes")])
}
fn run(inputs: &[&Path], output: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_taxaebook"))
        .arg("cover")
        .args(inputs)
        .arg("-o")
        .arg(output)
        .output()
        .unwrap()
}
fn assert_success(output: Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn versions_duplicates_and_collision() {
    let temp = tempfile::tempdir().unwrap();
    let book = temp.path().join("中文.EPUB");
    let out = temp.path().join("out");
    for version in [2, 3] {
        fs::write(&book, epub(version, true)).unwrap();
        assert_success(run(&[&book, &book], &out));
    }
    assert_eq!(fs::read_dir(&out).unwrap().count(), 2);
    for entry in fs::read_dir(out).unwrap() {
        assert_eq!(
            fs::read(entry.unwrap().path()).unwrap(),
            b"original cover bytes"
        );
    }
}
#[test]
fn recursive_directory_zip_and_path_traversal() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("books/nested");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("a.epub"), epub(2, true)).unwrap();
    fs::write(
        dir.join("books.zip"),
        zip(&[
            ("../../escape.epub", &epub(3, true)),
            ("ignore.txt", b"ignore"),
        ]),
    )
    .unwrap();
    let out = temp.path().join("out");
    assert_success(run(&[&temp.path().join("books")], &out));
    assert_eq!(fs::read_dir(out).unwrap().count(), 2);
    assert!(!temp.path().join("escape.epub").exists());
}
#[test]
fn tar_and_gzip() {
    let temp = tempfile::tempdir().unwrap();
    let bytes = epub(3, true);
    let mut builder = tar::Builder::new(Vec::new());
    let mut header = tar::Header::new_gnu();
    header.set_size(bytes.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    builder
        .append_data(&mut header, "nested/book.epub", bytes.as_slice())
        .unwrap();
    let tar = builder.into_inner().unwrap();
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(&tar).unwrap();
    let gzip = encoder.finish().unwrap();
    let out = temp.path().join("out");
    for (name, bytes) in [
        ("books.tar", tar),
        ("books.tar.gz", gzip.clone()),
        ("books.tgz", gzip),
    ] {
        let path = temp.path().join(name);
        fs::write(&path, bytes).unwrap();
        assert_success(run(&[&path], &out));
    }
    assert_eq!(fs::read_dir(out).unwrap().count(), 3);
}
#[test]
fn errors_do_not_stop_batch() {
    let temp = tempfile::tempdir().unwrap();
    let archive = temp.path().join("batch.zip");
    fs::write(
        &archive,
        zip(&[
            ("broken.epub", b"broken"),
            ("missing-cover.epub", &epub(3, false)),
            ("good.epub", &epub(2, true)),
        ]),
    )
    .unwrap();
    let out = temp.path().join("out");
    let result = run(&[&temp.path().join("absent.epub"), &archive], &out);
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(fs::read_dir(out).unwrap().count(), 1);
    assert!(String::from_utf8_lossy(&result.stderr).contains("成功 1，失败 3"));
}
#[test]
fn empty_unsupported_and_invalid_archive_fail() {
    let temp = tempfile::tempdir().unwrap();
    let out = temp.path().join("out");
    assert!(!run(&[temp.path()], &out).status.success());
    for name in ["bad.zip", "bad.tar", "bad.gz", "bad.rar"] {
        let file = temp.path().join(name);
        fs::write(&file, b"invalid").unwrap();
        assert!(!run(&[&file], &out).status.success());
    }
    assert_eq!(
        Command::new(env!("CARGO_BIN_EXE_taxaebook"))
            .output()
            .unwrap()
            .status
            .code(),
        Some(2)
    );
}
