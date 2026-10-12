use super::{
    client, endpoint, markdown, new_output, pdf::Manifest, read_json, secret, send, write_json,
};
use anyhow::{Context, Result, bail, ensure};
use reqwest::blocking::Client;
use serde_json::{Value, json};
use std::{
    fs::{self, File},
    io,
    path::Path,
    thread,
    time::{Duration, Instant},
};

fn data(response: Value) -> Result<Value> {
    ensure!(
        response["code"] == 0,
        "MinerU 错误 {}：{}",
        response["code"],
        response["msg"]
    );
    ensure!(response["data"].is_object(), "MinerU 响应缺少 data");
    Ok(response["data"].clone())
}

fn field<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value[key]
        .as_str()
        .filter(|s| !s.is_empty())
        .with_context(|| format!("MinerU 响应缺少 {key}"))
}

pub fn run(workspace: &Path, output: &Path, language: &str, poll: u64, timeout: u64) -> Result<()> {
    let manifest: Manifest = read_json(&workspace.join("manifest.json"))?;
    manifest.validate()?;
    // Check every input before spending any API quota.
    for (i, chapter) in manifest.chapters.iter().enumerate() {
        ensure!(
            manifest.end_page(i) - chapter.start_page < 200,
            "章节超过 200 页"
        );
        let path = workspace.join(format!("chapters/{:04}.pdf", i + 1));
        ensure!(fs::metadata(&path)?.len() <= 200_000_000, "章节超过 200 MB");
    }
    let token = secret("MINERU_API_TOKEN")?;
    let base = endpoint("MINERU_API_BASE_URL", "https://mineru.net/api/v4");
    let http = client()?;
    new_output(output)?;
    fs::create_dir(output.join("raw"))?;
    let mut merged = String::new();
    let mut failed = 0;
    for (i, chapter) in manifest.chapters.iter().enumerate() {
        let id = format!("{:04}", i + 1);
        let raw = output.join("raw").join(&id);
        fs::create_dir(&raw)?;
        eprintln!(
            "OCR {}/{}：{}",
            i + 1,
            manifest.chapters.len(),
            chapter.title
        );
        let result: Result<String> = (|| {
            let markdown_path = extract(
                &http,
                &base,
                &token,
                &workspace.join(format!("chapters/{id}.pdf")),
                &raw,
                language,
                poll,
                timeout,
            )?;
            let text = fs::read_to_string(raw.join("result").join(&markdown_path))?;
            ensure!(!text.trim().is_empty(), "MinerU 返回空 Markdown");
            let parent = markdown_path.parent().unwrap_or(Path::new(""));
            let prefix = format!(
                "raw/{id}/result/{}",
                parent.to_string_lossy().replace('\\', "/")
            );
            markdown::rebase(&text, prefix.trim_end_matches('/'))
        })();
        match result {
            Ok(text) => {
                // Keep OCR's headings untouched. The boundary records provenance without inventing hierarchy.
                merged.push_str(&format!(
                    "\n\n<!-- chapter {id}; PDF pages {}-{} -->\n\n",
                    chapter.start_page,
                    manifest.end_page(i)
                ));
                merged.push_str(&text);
                println!("{} -> {}", chapter.title, raw.display());
            }
            Err(error) => {
                failed += 1;
                fs::write(raw.join("error.txt"), format!("{error:#}"))?;
                eprintln!("OCR 失败 [{}]：{error:#}", chapter.title);
            }
        }
    }
    ensure!(
        failed == 0,
        "{failed} 章 OCR 失败；已保留成功结果和错误记录，不生成不完整的 source.md"
    );
    fs::write(output.join("source.md"), merged.trim_start())?;
    println!("合并源文 -> {}", output.join("source.md").display());
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn extract(
    http: &Client,
    base: &str,
    token: &str,
    input: &Path,
    raw: &Path,
    language: &str,
    poll: u64,
    timeout: u64,
) -> Result<std::path::PathBuf> {
    let name = input
        .file_name()
        .context("章节文件名缺失")?
        .to_string_lossy();
    let response: Value = send(
        http.post(format!("{base}/file-urls/batch"))
            .bearer_auth(token)
            .json(&json!({
                "files": [{"name": name, "data_id": "chapter", "is_ocr": true}],
                "model_version": "vlm", "language": language,
                "enable_formula": true, "enable_table": true
            })),
    )?
    .json()?;
    write_json(&raw.join("upload.json"), &response)?;
    let upload = data(response)?;
    let batch = field(&upload, "batch_id")?;
    ensure!(!batch.contains(['/', '?', '#']), "MinerU 返回非法 batch_id");
    let urls = upload["file_urls"]
        .as_array()
        .context("MinerU 缺少上传链接")?;
    ensure!(urls.len() == 1, "MinerU 上传链接数量错误");
    let url = urls[0].as_str().context("MinerU 上传链接格式错误")?;
    // Signed storage endpoints get neither the MinerU token nor an extra Content-Type.
    send(http.put(url).body(File::open(input)?))?;
    let deadline = Instant::now() + Duration::from_secs(timeout);
    let zip_url = loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        ensure!(!remaining.is_zero(), "MinerU OCR 等待超时（{timeout} 秒）");
        let response: Value = send(
            http.get(format!("{base}/extract-results/batch/{batch}"))
                .bearer_auth(token)
                .timeout(remaining.min(Duration::from_secs(300))),
        )?
        .json()?;
        write_json(&raw.join("status.json"), &response)?;
        let status = data(response)?;
        let entries = status["extract_result"]
            .as_array()
            .context("MinerU 缺少 extract_result")?;
        if let Some(entry) = entries
            .iter()
            .find(|e| e["file_name"].as_str() == Some(name.as_ref()))
        {
            match field(entry, "state")? {
                "done" => break field(entry, "full_zip_url")?.to_owned(),
                "failed" => bail!("MinerU 解析失败：{}", entry["err_msg"]),
                "waiting-file" | "pending" | "running" | "converting" => {}
                state => bail!("未知 MinerU 状态：{state}"),
            }
        }
        thread::sleep(
            Duration::from_secs(poll).min(deadline.saturating_duration_since(Instant::now())),
        );
    };
    let mut download = send(http.get(zip_url))?;
    io::copy(&mut download, &mut File::create(raw.join("result.zip"))?)?;
    unpack(&raw.join("result.zip"), &raw.join("result"))
}

fn unpack(zip: &Path, output: &Path) -> Result<std::path::PathBuf> {
    fs::create_dir(output)?;
    let mut archive = zip::ZipArchive::new(File::open(zip)?)?;
    let mut markdowns = Vec::new();
    let mut total = 0u64;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let path = entry.enclosed_name().context("MinerU ZIP 包含不安全路径")?;
        ensure!(
            !entry.name().contains(['\\', ':']),
            "MinerU ZIP 包含非跨平台路径"
        );
        ensure!(!entry.is_symlink(), "MinerU ZIP 不允许符号链接");
        total = total
            .checked_add(entry.size())
            .context("ZIP 解压大小溢出")?;
        ensure!(total <= 2_000_000_000, "MinerU ZIP 解压内容超过 2 GB");
        let destination = output.join(&path);
        if entry.is_dir() {
            fs::create_dir_all(&destination)?;
            continue;
        }
        fs::create_dir_all(destination.parent().context("ZIP 路径无父目录")?)?;
        let mut file = File::options()
            .write(true)
            .create_new(true)
            .open(&destination)?;
        io::copy(&mut entry, &mut file)?;
        if path.file_name().is_some_and(|n| n == "full.md") {
            markdowns.push(path);
        }
    }
    ensure!(
        markdowns.len() == 1,
        "MinerU ZIP 应包含唯一的 full.md，实际 {} 个；原始 ZIP 已保留",
        markdowns.len()
    );
    Ok(markdowns.remove(0))
}
