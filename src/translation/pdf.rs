use super::{new_output, read_json, write_json};
use anyhow::{Context, Result, ensure};
use lopdf::Document;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Chapter {
    pub title: String,
    pub start_page: u32,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub pages: u32,
    pub chapters: Vec<Chapter>,
}

impl Manifest {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.pages > 0 && !self.chapters.is_empty(),
            "PDF/章节表不能为空"
        );
        ensure!(
            self.chapters[0].start_page == 1,
            "章节表必须从物理第 1 页开始，避免遗漏前言"
        );
        let mut previous = 0;
        for chapter in &self.chapters {
            ensure!(!chapter.title.trim().is_empty(), "章节标题不能为空");
            ensure!(
                chapter.start_page > previous && chapter.start_page <= self.pages,
                "章节起始页必须严格递增且不超过总页数；同页分章暂不支持：{}",
                chapter.title
            );
            previous = chapter.start_page;
        }
        Ok(())
    }

    pub fn end_page(&self, index: usize) -> u32 {
        self.chapters
            .get(index + 1)
            .map_or(self.pages, |c| c.start_page - 1)
    }
}

pub fn prepare(
    input: &Path,
    output: &Path,
    chapters: Option<&Path>,
    level: u32,
    whole: bool,
) -> Result<()> {
    let document = Document::load(input).context("读取 PDF")?;
    ensure!(
        !document.is_encrypted(),
        "不支持加密 PDF，请先提供未加密文件"
    );
    let pages = u32::try_from(document.get_pages().len())?;
    let mut chapters: Vec<Chapter> = if let Some(path) = chapters {
        read_json(path)?
    } else if whole {
        vec![Chapter {
            title: "全文".into(),
            start_page: 1,
        }]
    } else {
        let toc = document
            .get_toc()
            .context("无法读取书签；请用 --chapters 提供章节表，或用 --whole 明确处理整本")?;
        ensure!(
            toc.errors.is_empty(),
            "书签解析有错误，请提供 --chapters：{:?}",
            toc.errors
        );
        let entries: Vec<_> = toc
            .toc
            .into_iter()
            .filter(|c| c.level == level as usize)
            .map(|c| {
                Ok(Chapter {
                    title: c.title,
                    start_page: u32::try_from(c.page)?,
                })
            })
            .collect::<Result<_>>()?;
        ensure!(
            !entries.is_empty(),
            "所选书签层级没有章节，请调整 --bookmark-level 或提供 --chapters"
        );
        entries
    };
    if chapters.first().is_some_and(|c| c.start_page > 1) {
        chapters.insert(
            0,
            Chapter {
                title: "前置页".into(),
                start_page: 1,
            },
        );
    }
    let manifest = Manifest { pages, chapters };
    manifest.validate()?;
    new_output(output)?;
    fs::create_dir(output.join("chapters"))?;
    for (i, chapter) in manifest.chapters.iter().enumerate() {
        let end = manifest.end_page(i);
        ensure!(
            end - chapter.start_page < 200,
            "章节“{}”超过 MinerU 当前 200 页上限，请细化章节表",
            chapter.title
        );
        let mut part = document.clone();
        part.delete_outlines()?;
        let removed: Vec<_> = (1..=pages)
            .filter(|p| *p < chapter.start_page || *p > end)
            .collect();
        part.delete_pages(&removed);
        part.prune_objects();
        let path = output.join(format!("chapters/{:04}.pdf", i + 1));
        part.save(&path)?;
        ensure!(
            fs::metadata(&path)?.len() <= 200_000_000,
            "章节“{}”超过 MinerU 200 MB 上限，请细化章节表",
            chapter.title
        );
        println!(
            "{}：{}–{} -> {}",
            chapter.title,
            chapter.start_page,
            end,
            path.display()
        );
    }
    write_json(&output.join("manifest.json"), &manifest)?;
    Ok(())
}
