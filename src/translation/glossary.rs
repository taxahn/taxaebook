use super::read_json;
use anyhow::{Result, ensure};
use std::{collections::BTreeMap, io::Write, path::Path};

pub type Glossary = BTreeMap<String, String>;

pub fn load(path: &Path) -> Result<Glossary> {
    let entries: Glossary = read_json(path)?;
    let mut terms = Glossary::new();
    for (source, target) in entries {
        add(&mut terms, source, target)?;
    }
    Ok(terms)
}

pub fn add(terms: &mut Glossary, source: String, target: String) -> Result<()> {
    ensure!(
        !source.trim().is_empty() && !target.trim().is_empty(),
        "术语和译名不能为空"
    );
    let normalized = source.to_lowercase();
    if let Some((_, old)) = terms
        .iter()
        .find(|(key, _)| key.to_lowercase() == normalized)
    {
        ensure!(old == &target, "术语冲突：{source}；已有译名未修改");
    } else {
        terms.insert(source, target);
    }
    Ok(())
}

pub fn merge(path: &Path, suggestions: &Path) -> Result<()> {
    let mut terms = if path.exists() {
        load(path)?
    } else {
        Glossary::new()
    };
    for (source, target) in load(suggestions)? {
        add(&mut terms, source, target)?;
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(&serde_json::to_vec_pretty(&terms)?)?;
    temporary.as_file().sync_all()?;
    temporary.persist(path)?;
    println!("{}：{} 个术语", path.display(), terms.len());
    Ok(())
}
