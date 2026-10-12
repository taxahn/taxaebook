use std::process::Command;

fn help(args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_taxaebook"))
        .args(args)
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn exposes_cover_and_translation_under_one_cli() {
    let root = help(&[]);
    assert!(root.contains("cover"));
    assert!(root.contains("translate"));
    assert!(help(&["cover"]).contains("--output"));
    let translation = help(&["translate"]);
    for command in ["prepare", "ocr", "markdown", "glossary-merge"] {
        assert!(translation.contains(command));
    }
    assert!(help(&["translate", "markdown"]).contains("--review"));
}

#[test]
fn requires_a_feature_and_a_translation_stage() {
    for args in [vec![], vec!["translate"]] {
        let output = Command::new(env!("CARGO_BIN_EXE_taxaebook"))
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
    }
}
