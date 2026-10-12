use lopdf::{Bookmark, Document, Object, Stream, dictionary};
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Cursor, Read, Write},
    net::{TcpListener, TcpStream},
    path::Path,
    process::{Command, Output},
    thread,
    time::{Duration, Instant},
};
use zip::{ZipWriter, write::SimpleFileOptions};

fn cli() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_taxaebook"));
    cmd.arg("translate")
        .env_remove("MINERU_API_TOKEN")
        .env_remove("DEEPSEEK_API_KEY")
        .env_remove("MINERU_API_BASE_URL")
        .env_remove("DEEPSEEK_API_BASE_URL")
        .env_remove("HTTP_PROXY")
        .env_remove("HTTPS_PROXY")
        .env_remove("ALL_PROXY")
        .env("NO_PROXY", "*");
    cmd
}

fn success(output: Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn pdf(path: &Path, bookmarks: bool) {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font = doc.add_object(
        dictionary! {"Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica"},
    );
    let resources = doc.add_object(dictionary! {"Font" => dictionary! {"F1" => font}});
    let mut pages = Vec::new();
    for i in 1..=4 {
        let content = doc.add_object(Stream::new(
            dictionary! {},
            format!("BT /F1 24 Tf 50 700 Td (Page {i}) Tj ET").into_bytes(),
        ));
        let page =
            doc.add_object(dictionary! {"Type"=>"Page", "Parent"=>pages_id, "Contents"=>content});
        pages.push(page);
    }
    doc.objects.insert(pages_id, dictionary! {"Type"=>"Pages", "Kids"=>pages.iter().copied().map(Object::Reference).collect::<Vec<_>>(),
        "Count"=>4, "Resources"=>resources, "MediaBox"=>vec![0.into(),0.into(),595.into(),842.into()]}.into());
    let catalog = doc.add_object(dictionary! {"Type"=>"Catalog", "Pages"=>pages_id});
    doc.trailer.set("Root", catalog);
    if bookmarks {
        for (title, page) in [("Chapter One", pages[1]), ("Chapter Two", pages[3])] {
            doc.add_bookmark(Bookmark::new(title.into(), [0.0; 3], 0, page), None);
        }
        let outlines = doc.build_outline().unwrap();
        doc.catalog_mut().unwrap().set("Outlines", outlines);
    }
    doc.save(path).unwrap();
}

fn prepare(root: &Path, bookmarks: bool) {
    pdf(&root.join("book.pdf"), bookmarks);
    let mut command = cli();
    command
        .arg("prepare")
        .arg(root.join("book.pdf"))
        .arg("-o")
        .arg(root.join("prepared"));
    if !bookmarks {
        command.arg("--whole");
    }
    success(command.output().unwrap());
}

#[test]
fn bookmark_split_keeps_front_matter_and_page_content() {
    let temp = tempfile::tempdir().unwrap();
    prepare(temp.path(), true);
    let manifest: Value =
        serde_json::from_slice(&fs::read(temp.path().join("prepared/manifest.json")).unwrap())
            .unwrap();
    assert_eq!(manifest["chapters"][0]["start_page"], 1);
    assert_eq!(manifest["chapters"][1]["start_page"], 2);
    assert_eq!(manifest["chapters"][2]["start_page"], 4);
    for (file, expected) in [
        ("0001.pdf", vec![1]),
        ("0002.pdf", vec![2, 3]),
        ("0003.pdf", vec![4]),
    ] {
        let part = Document::load(temp.path().join("prepared/chapters").join(file)).unwrap();
        assert_eq!(part.get_pages().len(), expected.len());
        let text = part
            .extract_text(&part.get_pages().keys().copied().collect::<Vec<_>>())
            .unwrap();
        for page in 1..=4 {
            assert_eq!(
                text.contains(&format!("Page {page}")),
                expected.contains(&page)
            );
        }
    }
    let before = fs::read(temp.path().join("prepared/chapters/0002.pdf")).unwrap();
    let output = cli()
        .arg("prepare")
        .arg(temp.path().join("book.pdf"))
        .arg("-o")
        .arg(temp.path().join("prepared"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(
        fs::read(temp.path().join("prepared/chapters/0002.pdf")).unwrap(),
        before
    );
}

#[test]
fn manual_chapters_reject_same_page_and_missing_outline_is_explicit() {
    let temp = tempfile::tempdir().unwrap();
    pdf(&temp.path().join("book.pdf"), false);
    fs::write(
        temp.path().join("chapters.json"),
        r#"[{"title":"a","start_page":1},{"title":"b","start_page":1}]"#,
    )
    .unwrap();
    let output = cli()
        .arg("prepare")
        .arg(temp.path().join("book.pdf"))
        .arg("--chapters")
        .arg(temp.path().join("chapters.json"))
        .arg("-o")
        .arg(temp.path().join("out"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!temp.path().join("out").exists());
    let output = cli()
        .arg("prepare")
        .arg(temp.path().join("book.pdf"))
        .arg("-o")
        .arg(temp.path().join("out"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--chapters"));
    fs::write(
        temp.path().join("chapters.json"),
        r#"[{"title":"a","start_page":1},{"title":"b","start_page":3}]"#,
    )
    .unwrap();
    success(
        cli()
            .arg("prepare")
            .arg(temp.path().join("book.pdf"))
            .arg("--chapters")
            .arg(temp.path().join("chapters.json"))
            .arg("-o")
            .arg(temp.path().join("out"))
            .output()
            .unwrap(),
    );
    assert_eq!(
        Document::load(temp.path().join("out/chapters/0002.pdf"))
            .unwrap()
            .get_pages()
            .len(),
        2
    );
}

struct Request {
    method: String,
    path: String,
    headers: String,
    body: Vec<u8>,
}
struct Response {
    status: u16,
    body: Vec<u8>,
}
fn response(value: Value) -> Response {
    Response {
        status: 200,
        body: serde_json::to_vec(&value).unwrap(),
    }
}

fn read_request(stream: &TcpStream) -> Request {
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let first: Vec<_> = line.split_whitespace().map(str::to_owned).collect();
    let mut headers = String::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        if line == "\r\n" {
            break;
        }
        assert!(!line.is_empty());
        headers.push_str(&line.to_lowercase());
    }
    let len = headers
        .lines()
        .find_map(|l| l.strip_prefix("content-length: "))
        .map(|v| v.parse().unwrap())
        .unwrap_or(0);
    let mut body = vec![0; len];
    reader.read_exact(&mut body).unwrap();
    Request {
        method: first[0].clone(),
        path: first[1].clone(),
        headers,
        body,
    }
}

fn server(
    count: usize,
    handler: impl Fn(usize, Request, &str) -> Response + Send + 'static,
) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let address = base.clone();
    let task = thread::spawn(move || {
        for i in 0..count {
            let deadline = Instant::now() + Duration::from_secs(15);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "mock API 等待第 {i} 个请求超时");
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(e) => panic!("{e}"),
                }
            };
            let result = handler(i, read_request(&stream), &address);
            write!(
                stream,
                "HTTP/1.1 {} OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                result.status,
                result.body.len()
            )
            .unwrap();
            stream.write_all(&result.body).unwrap();
        }
    });
    (base, task)
}

fn zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        writer
            .start_file(*name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn completion(markdown: &str, terms: Value, finish: &str) -> Response {
    response(
        json!({"choices":[{"finish_reason":finish,"message":{"content":json!({"markdown":markdown,"terms":terms}).to_string()}}]}),
    )
}

#[test]
fn pdf_to_ocr_to_reviewed_translation_and_reusable_glossary() {
    let temp = tempfile::tempdir().unwrap();
    prepare(temp.path(), false);
    let archive = zip(&[("full.md", b"# One\n\nNeural\n**network**.\n\n![figure](images/a.png)\n\n# Two\n\nNeural network again."),
        ("images/a.png", b"original image"), ("layout.json", b"{\"pages\":4}")]);
    let (base, task) = server(5, move |i, req, base| match i {
        0 => {
            assert_eq!(req.method, "POST");
            assert_eq!(req.path, "/api/v4/file-urls/batch");
            assert!(req.headers.contains("authorization: bearer mineru-test"));
            let body: Value = serde_json::from_slice(&req.body).unwrap();
            assert_eq!(body["model_version"], "vlm");
            assert_eq!(body["files"][0]["is_ocr"], true);
            response(
                json!({"code":0,"data":{"batch_id":"batch","file_urls":[format!("{base}/upload")]}}),
            )
        }
        1 => {
            assert_eq!(req.method, "PUT");
            assert_eq!(req.path, "/upload");
            assert!(!req.headers.contains("authorization:"));
            assert!(!req.headers.contains("content-type:"));
            assert!(req.body.starts_with(b"%PDF"));
            response(json!({}))
        }
        2 => response(
            json!({"code":0,"data":{"extract_result":[{"file_name":"0001.pdf","state":"running"}]}}),
        ),
        3 => response(
            json!({"code":0,"data":{"extract_result":[{"file_name":"0001.pdf","state":"done","full_zip_url":format!("{base}/result.zip")}]}}),
        ),
        4 => {
            assert!(!req.headers.contains("authorization:"));
            Response {
                status: 200,
                body: archive.clone(),
            }
        }
        _ => unreachable!(),
    });
    success(
        cli()
            .arg("ocr")
            .arg(temp.path().join("prepared"))
            .arg("-o")
            .arg(temp.path().join("ocr"))
            .args(["--poll-seconds", "1"])
            .env("MINERU_API_TOKEN", "mineru-test")
            .env("MINERU_API_BASE_URL", format!("{base}/api/v4"))
            .output()
            .unwrap(),
    );
    task.join().unwrap();
    assert_eq!(
        fs::read(temp.path().join("ocr/raw/0001/result/images/a.png")).unwrap(),
        b"original image"
    );
    assert!(temp.path().join("ocr/raw/0001/result/layout.json").exists());
    assert!(
        fs::read_to_string(temp.path().join("ocr/source.md"))
            .unwrap()
            .contains("raw/0001/result/images/a.png")
    );
    fs::write(temp.path().join("glossary.json"), "{}").unwrap();
    let (base, task) = server(4, |i, req, _| {
        assert_eq!(req.path, "/chat/completions");
        assert!(req.headers.contains("authorization: bearer deepseek-test"));
        let body: Value = serde_json::from_slice(&req.body).unwrap();
        assert_eq!(body["model"], "deepseek-flash");
        let request: Value =
            serde_json::from_str(body["messages"][1]["content"].as_str().unwrap()).unwrap();
        if i >= 2 {
            assert_eq!(request["glossary"]["Neural network"], "神经网络");
            assert!(
                request["previous_translation_tail"]
                    .as_str()
                    .unwrap()
                    .contains("神经网络")
            );
        }
        let source = request["source_markdown"].as_str().unwrap();
        let translated = source
            .replace("Neural\n**network**", "神经网络")
            .replace("Neural network", "神经网络")
            .replace("# One", "# 一")
            .replace("# Two", "# 二");
        if i % 2 == 1 {
            assert!(
                request["draft_markdown"]
                    .as_str()
                    .unwrap()
                    .contains("神经网络")
            );
        }
        completion(
            &translated,
            json!({
                "Neural network": if i >= 2 { "脑网络" } else { "神经网络" },
                "invented source term": "虚构术语"
            }),
            "stop",
        )
    });
    success(
        cli()
            .arg("markdown")
            .arg(temp.path().join("ocr/source.md"))
            .arg("-o")
            .arg(temp.path().join("translation"))
            .arg("--glossary")
            .arg(temp.path().join("glossary.json"))
            .arg("--review")
            .env("DEEPSEEK_API_KEY", "deepseek-test")
            .env("DEEPSEEK_API_BASE_URL", base)
            .output()
            .unwrap(),
    );
    task.join().unwrap();
    let translated = fs::read_to_string(temp.path().join("translation/translated.md")).unwrap();
    assert!(translated.contains("# 一"));
    assert!(translated.contains("# 二"));
    assert!(translated.contains("神经网络"));
    assert!(translated.contains("images/a.png"));
    let rejected: Value = serde_json::from_slice(
        &fs::read(temp.path().join("translation/terms.rejected.json")).unwrap(),
    )
    .unwrap();
    assert!(
        rejected
            .as_array()
            .unwrap()
            .iter()
            .any(|term| term["source"] == "invented source term")
    );
    assert!(
        rejected
            .as_array()
            .unwrap()
            .iter()
            .any(|term| term["source"] == "Neural network" && term["target"] == "脑网络")
    );
    assert!(
        temp.path()
            .join("translation/chunks/0001/draft.md")
            .exists()
    );
    assert_eq!(
        fs::read_to_string(temp.path().join("glossary.json")).unwrap(),
        "{}"
    );
    success(
        cli()
            .arg("glossary-merge")
            .arg(temp.path().join("glossary.json"))
            .arg(temp.path().join("translation/terms.suggested.json"))
            .output()
            .unwrap(),
    );
    let glossary: Value =
        serde_json::from_slice(&fs::read(temp.path().join("glossary.json")).unwrap()).unwrap();
    assert_eq!(glossary["Neural network"], "神经网络");
}

#[test]
fn glossary_conflict_does_not_change_existing_library() {
    let temp = tempfile::tempdir().unwrap();
    let glossary = temp.path().join("glossary.json");
    let suggestions = temp.path().join("suggestions.json");
    fs::write(&glossary, r#"{"network":"网络"}"#).unwrap();
    fs::write(&suggestions, r#"{"added":"新增","Network":"网路"}"#).unwrap();
    assert!(
        !cli()
            .arg("glossary-merge")
            .arg(&glossary)
            .arg(&suggestions)
            .output()
            .unwrap()
            .status
            .success()
    );
    assert_eq!(
        fs::read_to_string(glossary).unwrap(),
        r#"{"network":"网络"}"#
    );
}

#[test]
fn truncated_translation_keeps_response_but_never_emits_complete_book() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("source.md"), "# Source\n\nText.").unwrap();
    let (base, task) = server(1, |_, _, _| completion("# 译文", json!({}), "length"));
    let output = cli()
        .arg("markdown")
        .arg(temp.path().join("source.md"))
        .arg("-o")
        .arg(temp.path().join("out"))
        .env("DEEPSEEK_API_KEY", "test")
        .env("DEEPSEEK_API_BASE_URL", base)
        .output()
        .unwrap();
    task.join().unwrap();
    assert!(!output.status.success());
    assert!(
        temp.path()
            .join("out/chunks/0001/draft.response.json")
            .exists()
    );
    assert!(!temp.path().join("out/translated.md").exists());
}

#[test]
fn translation_without_optional_term_suggestions_succeeds() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("source.md"), "# Source\n\nText.").unwrap();
    let (base, task) = server(1, |_, _, _| {
        response(json!({"choices":[{"finish_reason":"stop", "message":{
            "content":serde_json::to_string(&json!({"markdown":"# 译文\n\n正文。"})).unwrap()
        }}]}))
    });
    success(
        cli()
            .arg("markdown")
            .arg(temp.path().join("source.md"))
            .arg("-o")
            .arg(temp.path().join("out"))
            .env("DEEPSEEK_API_KEY", "test")
            .env("DEEPSEEK_API_BASE_URL", base)
            .output()
            .unwrap(),
    );
    task.join().unwrap();
    assert_eq!(
        fs::read_to_string(temp.path().join("out/translated.md")).unwrap(),
        "# 译文\n\n正文。"
    );
    let suggestions: Value =
        serde_json::from_slice(&fs::read(temp.path().join("out/terms.suggested.json")).unwrap())
            .unwrap();
    assert_eq!(suggestions, json!({}));
}

#[test]
fn invalid_term_metadata_is_recorded_without_discarding_translation() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("source.md"), "Text.").unwrap();
    let (base, task) = server(1, |_, _, _| {
        completion(
            "正文。",
            json!([{"source":"Text", "target":"正文"}]),
            "stop",
        )
    });
    success(
        cli()
            .arg("markdown")
            .arg(temp.path().join("source.md"))
            .arg("-o")
            .arg(temp.path().join("out"))
            .env("DEEPSEEK_API_KEY", "test")
            .env("DEEPSEEK_API_BASE_URL", base)
            .output()
            .unwrap(),
    );
    task.join().unwrap();
    assert_eq!(
        fs::read_to_string(temp.path().join("out/translated.md")).unwrap(),
        "正文。"
    );
    let rejected: Value =
        serde_json::from_slice(&fs::read(temp.path().join("out/terms.rejected.json")).unwrap())
            .unwrap();
    assert_eq!(
        rejected[0]["terms"],
        json!([{"source":"Text", "target":"正文"}])
    );
}

#[test]
fn resume_reuses_completed_responses_and_preserves_context() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.md");
    let out = temp.path().join("out");
    fs::write(&source, "# One\n\nTerm.\n\n# Two\n\nTerm.").unwrap();
    let (base, task) = server(1, |_, _, _| {
        completion("# 一\n\n术语。", json!({"Term":"术语"}), "stop")
    });
    let first = cli()
        .arg("markdown")
        .arg(&source)
        .arg("-o")
        .arg(&out)
        .env("DEEPSEEK_API_KEY", "test")
        .env("DEEPSEEK_API_BASE_URL", base)
        .output()
        .unwrap();
    task.join().unwrap();
    assert!(!first.status.success());
    assert!(!out.join("translated.md").exists());
    let first_response = fs::read(out.join("chunks/0001/draft.response.json")).unwrap();
    let (base, task) = server(1, |_, req, _| {
        let body: Value = serde_json::from_slice(&req.body).unwrap();
        let request: Value =
            serde_json::from_str(body["messages"][1]["content"].as_str().unwrap()).unwrap();
        assert!(
            request["source_markdown"]
                .as_str()
                .unwrap()
                .contains("# Two")
        );
        assert_eq!(request["glossary"]["Term"], "术语");
        assert!(
            request["previous_translation_tail"]
                .as_str()
                .unwrap()
                .contains("# 一")
        );
        completion("# 二\n\n术语。", json!({"Term":"术语"}), "stop")
    });
    success(
        cli()
            .arg("markdown")
            .arg(&source)
            .arg("-o")
            .arg(&out)
            .arg("--resume")
            .env("DEEPSEEK_API_KEY", "test")
            .env("DEEPSEEK_API_BASE_URL", &base)
            .output()
            .unwrap(),
    );
    task.join().unwrap();
    assert_eq!(
        fs::read(out.join("chunks/0001/draft.response.json")).unwrap(),
        first_response
    );
    let translated = fs::read_to_string(out.join("translated.md")).unwrap();
    assert_eq!(translated, "# 一\n\n术语。\n\n# 二\n\n术语。");
    let mismatch = cli()
        .arg("markdown")
        .arg(&source)
        .arg("-o")
        .arg(&out)
        .args(["--resume", "--to", "English"])
        .env("DEEPSEEK_API_KEY", "test")
        .env("DEEPSEEK_API_BASE_URL", &base)
        .output()
        .unwrap();
    assert!(!mismatch.status.success());
    assert!(String::from_utf8_lossy(&mismatch.stderr).contains("续跑参数"));
    fs::write(&source, "changed source").unwrap();
    let changed = cli()
        .arg("markdown")
        .arg(&source)
        .arg("-o")
        .arg(&out)
        .arg("--resume")
        .env("DEEPSEEK_API_KEY", "test")
        .env("DEEPSEEK_API_BASE_URL", &base)
        .output()
        .unwrap();
    assert!(!changed.status.success());
    assert_eq!(
        fs::read_to_string(out.join("translated.md")).unwrap(),
        translated
    );
}

#[test]
fn missing_key_is_explained_without_creating_output() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("source.md"), "hello").unwrap();
    let output = cli()
        .arg("markdown")
        .arg(temp.path().join("source.md"))
        .arg("-o")
        .arg(temp.path().join("out"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("DEEPSEEK_API_KEY"));
    assert!(!temp.path().join("out").exists());
}

#[test]
fn ocr_failure_continues_next_chapter_and_rejects_invalid_results() {
    for archive in [
        zip(&[("../escape.md", b"unsafe"), ("full.md", b"text")]),
        zip(&[("full.md", b"\xff")]),
        zip(&[("full.md", b"")]),
    ] {
        let temp = tempfile::tempdir().unwrap();
        prepare(temp.path(), true);
        let (base, task) = server(10, move |i, req, base| {
            let chapter = if i < 3 {
                1
            } else if i < 7 {
                2
            } else {
                3
            };
            if req.path.ends_with("file-urls/batch") {
                return response(
                    json!({"code":0,"data":{"batch_id":format!("b{chapter}"),"file_urls":[format!("{base}/upload")]}}),
                );
            }
            if req.method == "PUT" {
                return response(json!({}));
            }
            if req.path == "/result.zip" {
                return Response {
                    status: 200,
                    body: archive.clone(),
                };
            }
            let entry = if chapter == 2 {
                json!({"file_name":"0002.pdf","state":"done","full_zip_url":format!("{base}/result.zip")})
            } else {
                json!({"file_name":format!("{chapter:04}.pdf"),"state":"failed","err_msg":"test error"})
            };
            response(json!({"code":0,"data":{"extract_result":[entry]}}))
        });
        let output = cli()
            .arg("ocr")
            .arg(temp.path().join("prepared"))
            .arg("-o")
            .arg(temp.path().join("out"))
            .env("MINERU_API_TOKEN", "test")
            .env("MINERU_API_BASE_URL", format!("{base}/api/v4"))
            .output()
            .unwrap();
        task.join().unwrap();
        assert!(!output.status.success());
        assert!(!temp.path().join("out/source.md").exists());
        assert!(!temp.path().join("out/raw/0002/escape.md").exists());
        for i in 1..=3 {
            assert!(
                temp.path()
                    .join(format!("out/raw/{i:04}/error.txt"))
                    .exists()
            );
        }
        assert!(temp.path().join("out/raw/0002/result.zip").exists());
    }
}
