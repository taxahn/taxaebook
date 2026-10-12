use super::{glossary, llm, ocr, pdf};

use anyhow::Result;
use clap::{Args as ClapArgs, Subcommand};
use std::path::PathBuf;

#[derive(ClapArgs)]
pub struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 按 PDF 书签或手工章节表切书；只在本地运行
    Prepare {
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        /// JSON 数组：[{"title":"第一章","start_page":1}]，物理页码从 1 开始
        #[arg(long, conflicts_with = "whole")]
        chapters: Option<PathBuf>,
        /// 使用这一层级的 PDF 书签作为章节边界
        #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..))]
        bookmark_level: u32,
        /// 明确将整本书作为单个章节（仍受 MinerU 大小/页数限制）
        #[arg(long)]
        whole: bool,
    },
    /// 上传切好的章节并保留 MinerU 完整 ZIP/JSON/图片，合并为 source.md
    Ocr {
        workspace: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        /// MinerU 的源语言代码，例如 en、ch
        #[arg(long, default_value = "en")]
        language: String,
        #[arg(long, default_value_t = 5, value_parser = clap::value_parser!(u64).range(1..))]
        poll_seconds: u64,
        /// 每章等待 OCR 的最长秒数
        #[arg(long, default_value_t = 1800, value_parser = clap::value_parser!(u64).range(1..))]
        timeout_seconds: u64,
    },
    /// 翻译 Markdown；独立于 OCR，也可直接处理已有 Markdown
    Markdown {
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        /// 长期术语库 JSON：{"source term":"译名"}
        #[arg(long)]
        glossary: Option<PathBuf>,
        #[arg(long, default_value = "简体中文")]
        to: String,
        /// 每块源文的 Unicode 字符上限；不会从表格、列表或代码块中间切开
        #[arg(long, default_value_t = 6000, value_parser = clap::value_parser!(u32).range(256..=20000))]
        chunk_chars: u32,
        /// 每块翻译后再调用一次审校；保留初稿和审校结果供比较
        #[arg(long)]
        review: bool,
        /// 校验源文和参数后复用已保存的模型响应，继续同一输出目录
        #[arg(long)]
        resume: bool,
    },
    /// 将检查过的术语建议合并到长期库；冲突时报错，不覆盖已有译名
    GlossaryMerge {
        glossary: PathBuf,
        suggestions: PathBuf,
    },
}

pub fn run(args: Args) -> Result<()> {
    match args.command {
        Command::Prepare {
            input,
            output,
            chapters,
            bookmark_level,
            whole,
        } => pdf::prepare(&input, &output, chapters.as_deref(), bookmark_level, whole),
        Command::Ocr {
            workspace,
            output,
            language,
            poll_seconds,
            timeout_seconds,
        } => ocr::run(
            &workspace,
            &output,
            &language,
            poll_seconds,
            timeout_seconds,
        ),
        Command::Markdown {
            input,
            output,
            glossary,
            to,
            chunk_chars,
            review,
            resume,
        } => llm::run(
            &input,
            &output,
            glossary.as_deref(),
            &to,
            chunk_chars as usize,
            review,
            resume,
        ),
        Command::GlossaryMerge {
            glossary,
            suggestions,
        } => glossary::merge(&glossary, &suggestions),
    }
}
