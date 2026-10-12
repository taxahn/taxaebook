use anyhow::{Context, Result};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

#[derive(Clone, Copy)]
pub enum Kind {
    Epub,
    Zip,
    Tar,
    Gzip,
}

pub fn kind(path: &Path) -> Option<Kind> {
    let name = path.file_name()?.to_str()?.to_ascii_lowercase();
    if name.ends_with(".epub") {
        Some(Kind::Epub)
    } else if name.ends_with(".zip") {
        Some(Kind::Zip)
    } else if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        Some(Kind::Gzip)
    } else if name.ends_with(".tar") {
        Some(Kind::Tar)
    } else {
        None
    }
}

// Spool one EPUB at a time, without extracting archive paths onto disk.
fn spool(mut reader: impl Read) -> Result<File> {
    let mut file = tempfile::tempfile()?;
    std::io::copy(&mut reader, &mut file)?;
    file.seek(SeekFrom::Start(0))?;
    Ok(file)
}

pub fn visit(path: &Path, mut book: impl FnMut(&Path, Result<File>)) -> Result<usize> {
    let file = File::open(path).context("无法打开输入文件")?;
    let mut count = 0;
    match kind(path).context("不支持的输入格式（支持 EPUB、ZIP、TAR、TAR.GZ、TGZ）")?
    {
        Kind::Epub => {
            book(path, Ok(file));
            count += 1;
        }
        Kind::Zip => {
            let mut archive = zip::ZipArchive::new(file).context("无法解析 ZIP")?;
            for index in 0..archive.len() {
                let mut entry = archive.by_index(index)?;
                let name = std::path::PathBuf::from(entry.name().replace('\\', "/"));
                if entry.is_file() && matches!(kind(&name), Some(Kind::Epub)) {
                    book(&name, spool(&mut entry));
                    count += 1;
                }
            }
        }
        archive_kind @ (Kind::Tar | Kind::Gzip) => {
            let reader: Box<dyn Read> = match archive_kind {
                Kind::Gzip => Box::new(flate2::read::MultiGzDecoder::new(file)),
                _ => Box::new(file),
            };
            let mut archive = tar::Archive::new(reader);
            for entry in archive.entries()? {
                let mut entry = entry?;
                let name = entry.path()?.into_owned();
                if entry.header().entry_type().is_file() && matches!(kind(&name), Some(Kind::Epub))
                {
                    book(&name, spool(&mut entry));
                    count += 1;
                }
            }
        }
    }
    Ok(count)
}
