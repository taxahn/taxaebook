use super::{extract, input};

use anyhow::Result;
use clap::Args as ClapArgs;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    process::ExitCode,
};

#[derive(ClapArgs)]
pub struct Args {
    /// EPUB、目录、ZIP、TAR、TAR.GZ 或 TGZ；可混合传入多个路径
    #[arg(required = true, value_name = "INPUT")]
    inputs: Vec<PathBuf>,
    /// 输出目录；同名文件自动编号，不覆盖已有文件
    #[arg(short, long, default_value = "covers", value_name = "DIR")]
    output: PathBuf,
}

#[derive(Default)]
struct Batch {
    success: usize,
    failed: usize,
    seen: HashSet<PathBuf>,
}
impl Batch {
    fn error(&mut self, source: &Path, error: impl std::fmt::Display) {
        self.failed += 1;
        eprintln!("错误 [{}]: {error}", source.display());
    }
    fn file(&mut self, source: &Path, output: &Path) {
        let canonical = match source.canonicalize() {
            Ok(path) => path,
            Err(error) => {
                self.error(source, error);
                return;
            }
        };
        if !self.seen.insert(canonical) {
            return;
        }
        let result = input::visit(source, |name, reader| {
            let result: Result<_> = (|| {
                let (bytes, extension) = extract::extract(reader?)?;
                extract::save(output, name, &bytes, extension)
            })();
            match result {
                Ok(path) => {
                    self.success += 1;
                    println!(
                        "{} :: {} -> {}",
                        source.display(),
                        name.display(),
                        path.display()
                    );
                }
                Err(error) => self.error(source, format!("{}: {error:#}", name.display())),
            }
        });
        match result {
            Ok(0) => self.error(source, "未找到 EPUB"),
            Err(error) => self.error(source, format!("{error:#}")),
            _ => {}
        }
    }
}

pub fn run(args: Args) -> ExitCode {
    let mut batch = Batch::default();
    for input in args.inputs {
        if input.is_dir() {
            let mut found = false;
            for entry in walkdir::WalkDir::new(&input).sort_by_file_name() {
                match entry {
                    Ok(entry)
                        if entry.file_type().is_file() && input::kind(entry.path()).is_some() =>
                    {
                        found = true;
                        batch.file(entry.path(), &args.output);
                    }
                    Err(error) => batch.error(&input, error),
                    _ => {}
                }
            }
            if !found {
                batch.error(&input, "目录中未找到支持的文件");
            }
        } else {
            batch.file(&input, &args.output);
        }
    }
    eprintln!("完成：成功 {}，失败 {}", batch.success, batch.failed);
    if batch.failed == 0 && batch.success > 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
