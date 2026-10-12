use anyhow::{Context, Result, bail};
use epub::doc::EpubDoc;
use std::{
    io::Write,
    io::{Read, Seek},
    path::{Path, PathBuf},
};

pub fn extract(reader: impl Read + Seek) -> Result<(Vec<u8>, &'static str)> {
    let mut book = EpubDoc::from_reader(reader).context("无法解析 EPUB")?;
    let (bytes, mime) = book
        .get_cover()
        .context("EPUB 未声明封面或封面资源无法读取")?;
    let extension = match mime.as_str() {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/svg+xml" => "svg",
        "image/avif" => "avif",
        "image/bmp" => "bmp",
        "image/tiff" => "tiff",
        _ => bail!("不支持的封面媒体类型: {mime}"),
    };
    anyhow::ensure!(!bytes.is_empty(), "封面资源为空");
    Ok((bytes, extension))
}

pub fn save(directory: &Path, name: &Path, bytes: &[u8], extension: &str) -> Result<PathBuf> {
    std::fs::create_dir_all(directory).context("无法创建输出目录")?;
    // A portable, bounded basename; archive paths never become output paths.
    let stem = name.file_stem().context("文件名为空")?.to_string_lossy();
    let stem: String = stem
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .take(60)
        .collect();
    let stem = stem.trim_end_matches(['.', ' ']);
    let stem = if stem.is_empty() { "cover" } else { stem };
    let mut temporary = tempfile::NamedTempFile::new_in(directory)?;
    temporary.write_all(bytes)?;
    for index in 1u64.. {
        // Prefix avoids reserved Windows device names such as CON and AUX.
        let suffix = if index == 1 {
            String::new()
        } else {
            format!("-{index}")
        };
        let path = directory.join(format!("cover-{stem}{suffix}.{extension}"));
        match temporary.persist_noclobber(&path) {
            Ok(_) => return Ok(path),
            Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
                temporary = error.file
            }
            Err(error) => return Err(error.error.into()),
        }
    }
    unreachable!()
}
