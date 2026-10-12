mod cover;
mod translation;

use clap::{Parser, Subcommand};
use std::process::ExitCode;

#[derive(Parser)]
#[command(version, about = "电子书工具：EPUB 封面提取与 AI 辅助翻译")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 从 EPUB 文件、目录或压缩包提取原始封面
    Cover(cover::Args),
    /// 按章节准备 PDF、调用 MinerU OCR、使用 DeepSeek 翻译 Markdown
    Translate(translation::Args),
}

fn main() -> ExitCode {
    match Args::parse().command {
        Command::Cover(args) => cover::run(args),
        Command::Translate(args) => match translation::run(args) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("错误：{error:#}");
                ExitCode::FAILURE
            }
        },
    }
}
