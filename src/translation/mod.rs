mod cli;
pub use cli::{Args, run};

mod glossary;
mod llm;
mod markdown;
mod ocr;
mod pdf;

use anyhow::{Context, Result, ensure};
use reqwest::blocking::{Client, RequestBuilder, Response};
use serde::{Serialize, de::DeserializeOwned};
use std::{fs, path::Path, time::Duration};

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    serde_json::from_slice(&fs::read(path).with_context(|| format!("读取 {}", path.display()))?)
        .with_context(|| format!("解析 {}", path.display()))
}

pub fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}

pub fn new_output(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(path).with_context(|| {
        format!(
            "创建输出目录 {}（必须不存在，避免覆盖结果）",
            path.display()
        )
    })?;
    Ok(())
}

pub fn client() -> Result<Client> {
    Ok(Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(300))
        .build()?)
}

pub fn secret(name: &str) -> Result<String> {
    let value = std::env::var(name).with_context(|| format!("请先设置环境变量 {name}"))?;
    ensure!(!value.trim().is_empty(), "环境变量 {name} 不能为空");
    Ok(value)
}

pub fn endpoint(name: &str, default: &str) -> String {
    std::env::var(name)
        .unwrap_or_else(|_| default.into())
        .trim_end_matches('/')
        .into()
}

pub fn send(request: RequestBuilder) -> Result<Response> {
    // Signed upload/download URLs must not leak into diagnostics.
    let response = request.send().map_err(reqwest::Error::without_url)?;
    ensure!(
        response.status().is_success(),
        "API HTTP {}",
        response.status()
    );
    Ok(response)
}
