use anyhow::{Result, ensure};
use pulldown_cmark::{Event, LinkType, Options, Parser, Tag, TagEnd};

pub fn normalize(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Match terms against readable text, including phrases wrapped across lines or emphasis.
pub fn search_text(markdown: &str) -> String {
    let mut text = String::new();
    for event in Parser::new_ext(markdown, Options::all()) {
        match event {
            Event::Text(value)
            | Event::Code(value)
            | Event::Html(value)
            | Event::InlineHtml(value)
            | Event::InlineMath(value)
            | Event::DisplayMath(value) => {
                text.push_str(&value);
            }
            Event::SoftBreak
            | Event::HardBreak
            | Event::Rule
            | Event::End(
                TagEnd::Paragraph
                | TagEnd::Heading(_)
                | TagEnd::Item
                | TagEnd::CodeBlock
                | TagEnd::TableCell
                | TagEnd::TableRow
                | TagEnd::FootnoteDefinition,
            ) => text.push(' '),
            _ => {}
        }
    }
    normalize(&text)
}

pub fn rebase(text: &str, prefix: &str) -> Result<String> {
    let events = Parser::new_ext(text, Options::all()).map(|event| match event {
        Event::Start(Tag::Image {
            link_type: _,
            dest_url,
            title,
            id,
        }) => Event::Start(Tag::Image {
            link_type: LinkType::Inline,
            dest_url: rebase_url(&dest_url, prefix).into(),
            title,
            id,
        }),
        Event::Start(Tag::Link {
            link_type: _,
            dest_url,
            title,
            id,
        }) => Event::Start(Tag::Link {
            link_type: LinkType::Inline,
            dest_url: rebase_url(&dest_url, prefix).into(),
            title,
            id,
        }),
        other => other,
    });
    let mut result = String::new();
    pulldown_cmark_to_cmark::cmark(events, &mut result)?;
    Ok(result)
}

fn rebase_url(url: &str, prefix: &str) -> String {
    if url.is_empty() || url.starts_with(['#', '/']) || url.contains(':') {
        url.into()
    } else {
        format!("{prefix}/{url}")
    }
}

/// Keep top-level Markdown blocks intact and prefer headings as boundaries.
pub fn chunks(text: &str, limit: usize) -> Result<Vec<String>> {
    ensure!(!text.trim().is_empty(), "Markdown 不能为空");
    let mut boundaries = vec![(0, false)];
    let mut depth = 0;
    for (event, range) in Parser::new_ext(text, Options::all()).into_offset_iter() {
        match event {
            Event::Start(tag) => {
                if depth == 0 {
                    boundaries.push((range.start, matches!(tag, Tag::Heading { .. })));
                }
                depth += 1;
            }
            Event::End(_) => {
                depth -= 1;
            }
            _ if depth == 0 => {
                boundaries.push((range.start, false));
            }
            _ => {}
        }
    }
    boundaries.push((text.len(), false));
    boundaries.sort_by_key(|b| b.0);
    boundaries.dedup_by_key(|b| b.0);
    let mut result = Vec::new();
    let mut current = String::new();
    let mut count = 0;
    for pair in boundaries.windows(2) {
        let block = &text[pair[0].0..pair[1].0];
        let size = block.chars().count();
        ensure!(
            size <= limit,
            "单个 Markdown 块有 {size} 字符，超过 --chunk-chars {limit}；请调整上限或手工拆分该段落/表格/代码块"
        );
        let heading_boundary = pair[0].1
            && Parser::new_ext(&current, Options::all()).any(|event| {
                matches!(
                    event,
                    Event::Text(_) | Event::Code(_) | Event::Start(Tag::Image { .. })
                )
            });
        if !current.is_empty() && (heading_boundary || count + size > limit) {
            result.push(std::mem::take(&mut current));
            count = 0;
        }
        current.push_str(block);
        count += size;
    }
    if !current.trim().is_empty() {
        result.push(current);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keeps_tables_code_lists_and_unicode_intact() {
        let text = "# 一\n\n段落。\n\n```rust\n# not heading\n```\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\n# 二\n\n- a\n- b\n";
        let parts = chunks(text, 64).unwrap();
        assert_eq!(parts.concat(), text);
        assert!(
            parts
                .iter()
                .any(|p| p.contains("```rust\n# not heading\n```"))
        );
        assert!(
            parts
                .iter()
                .any(|p| p.contains("| A | B |\n|---|---|\n| 1 | 2 |"))
        );
        assert!(parts.iter().any(|p| p.contains("- a\n- b")));
        assert!(parts.iter().all(|p| p.chars().count() <= 64));
        assert!(chunks(&"x".repeat(65), 64).is_err());
    }
    #[test]
    fn resolves_reference_images_when_rebasing() {
        let result = rebase(
            "![图][pic]\n\n[pic]: images/a.png\n\n[web](https://example.com)",
            "raw/0001/result",
        )
        .unwrap();
        assert!(result.contains("raw/0001/result/images/a.png"));
        assert!(result.contains("https://example.com"));
        assert!(!result.contains("[pic]"));
    }
}
