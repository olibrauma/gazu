//! gazu CLI 統合テスト。
//!
//! sekien は lib として埋め込まれているため、別途インストールは不要。
//! Linux では Xvfb が必要 (sekien が内部で起動する)。

use std::ffi::OsString;
use std::io::Write;
use std::process::{Command, Output, Stdio};

fn gazu_bin() -> OsString {
    env!("CARGO_BIN_EXE_gazu").into()
}

fn pandoc_available() -> bool {
    Command::new("pandoc")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

macro_rules! pandoc_or_skip {
    () => {
        if !pandoc_available() {
            println!("(skip) pandoc not found");
            return;
        }
    };
}

/// pandoc を `-t html --filter gazu` 付きで呼び出す。
fn run_pandoc(markdown: &str) -> Output {
    run_pandoc_in(None, "html", &[], markdown)
}

/// pandoc を `-t <to> --filter gazu` 付きで呼び出す。
/// `dir` を指定すると pandoc の CWD をそこに変更する
/// (gazu が typst 等向けに SVG ファイルを書き出す先になる)。
/// `envs` は gazu に渡す追加の環境変数 (例: `GAZU_CONFIG`)。
fn run_pandoc_in(
    dir: Option<&std::path::Path>,
    to: &str,
    envs: &[(&str, &str)],
    markdown: &str,
) -> Output {
    let mut cmd = Command::new("pandoc");
    cmd.args(["-f", "markdown", "-t", to, "--filter"])
        .arg(gazu_bin())
        .envs(envs.iter().copied())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(dir) = dir {
        cmd.current_dir(dir);
    }
    let mut child = cmd.spawn().expect("spawn pandoc");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(markdown.as_bytes())
        .expect("write stdin");
    child.wait_with_output().expect("wait pandoc")
}

// ── CLI flags ─────────────────────────────────────────────────────────────────

#[test]
fn help_exits_zero() {
    let out = Command::new(gazu_bin())
        .arg("--help")
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("gazu"), "help missing 'gazu': {stdout}");
}

#[test]
fn version_exits_zero() {
    let out = Command::new(gazu_bin())
        .arg("--version")
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("gazu"),
        "--version missing 'gazu': {stdout}"
    );
    assert!(
        stdout.contains("mermaid.js"),
        "--version missing 'mermaid.js': {stdout}"
    );
}

#[test]
fn unknown_flag_is_ignored() {
    // Pandoc が output format 名を渡すケースを模倣する
    let out = Command::new(gazu_bin())
        .arg("html")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    // stdin を閉じて EOF を送る (Pandoc AST が来ない = エラーになるが exit 非 0 のみ確認)
    let output = out.wait_with_output().expect("wait");
    // exit 1 は JSON parse 失敗によるもので、unknown flag によるものではない
    // "unknown option" のようなメッセージが stderr に出ていないことを確認する
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("unknown"),
        "unexpected error for unknown flag: {stderr}"
    );
}

// ── Pandoc integration ────────────────────────────────────────────────────────

#[test]
fn pandoc_converts_mermaid_to_svg() {
    pandoc_or_skip!();
    let md = "# test\n\n```mermaid\ngraph LR\n  A --> B\n```\n";
    let out = run_pandoc(md);
    assert!(
        out.status.success(),
        "pandoc failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let html = String::from_utf8_lossy(&out.stdout);
    assert!(html.contains("<svg"), "no SVG in output: {html}");
}

#[test]
fn pandoc_passes_through_non_mermaid() {
    pandoc_or_skip!();
    let md = "# hello\n\nsome text\n";
    let out = run_pandoc(md);
    assert!(out.status.success());
    let html = String::from_utf8_lossy(&out.stdout);
    assert!(html.contains("hello"), "non-mermaid content lost: {html}");
    assert!(!html.contains("<svg"), "unexpected SVG: {html}");
}

#[test]
fn pandoc_handles_multiple_mermaid_blocks() {
    pandoc_or_skip!();
    let md = "```mermaid\ngraph LR\n  A --> B\n```\n\n```mermaid\ngraph TD\n  X --> Y\n```\n";
    let out = run_pandoc(md);
    assert!(out.status.success());
    let html = String::from_utf8_lossy(&out.stdout);
    let count = html.matches("<svg").count();
    assert!(count >= 2, "expected >=2 SVGs, got {count}: {html}");
}

#[test]
fn pandoc_partial_failure_keeps_fallback() {
    pandoc_or_skip!();
    let md = "```mermaid\ngraph LR\n  A --> B\n```\n\n\
              ```mermaid\ntotallyBogusDiagram\n```\n\n\
              ```mermaid\ngraph TD\n  X --> Y\n```\n";
    let out = run_pandoc(md);
    assert!(out.status.success());
    let html = String::from_utf8_lossy(&out.stdout);
    let svg_count = html.matches("<svg").count();
    assert_eq!(svg_count, 2, "expected 2 SVGs, got {svg_count}");
    assert!(
        html.contains("totallyBogusDiagram"),
        "failed mermaid block should remain as code: {html}"
    );
}

#[test]
fn pandoc_converts_mermaid_inside_div() {
    pandoc_or_skip!();
    let md = "::: note\n```mermaid\ngraph LR\n  A --> B\n```\n:::\n";
    let out = run_pandoc(md);
    assert!(out.status.success());
    let html = String::from_utf8_lossy(&out.stdout);
    assert!(
        html.contains("<svg"),
        "Mermaid inside Div not converted: {html}"
    );
}

#[test]
fn gazu_config_applies_mermaid_theme() {
    pandoc_or_skip!();
    let dir = std::env::temp_dir().join(format!("gazu-cfg-{:?}", std::thread::current().id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let config_path = dir.join("mermaid-config.json");
    std::fs::write(&config_path, r#"{"theme":"dark"}"#).expect("write config");

    let md = "```mermaid\ngraph LR\n  A --> B\n```\n";
    let out = run_pandoc_in(
        None,
        "html",
        &[("GAZU_CONFIG", config_path.to_str().unwrap())],
        md,
    );
    assert!(
        out.status.success(),
        "pandoc failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let html = String::from_utf8_lossy(&out.stdout);
    assert!(html.contains("#1f2020"), "dark theme not applied: {html}");

    std::fs::remove_dir_all(&dir).expect("cleanup temp dir");
}

#[test]
fn gazu_config_invalid_json_fails() {
    pandoc_or_skip!();
    let dir = std::env::temp_dir().join(format!("gazu-cfg-bad-{:?}", std::thread::current().id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let config_path = dir.join("mermaid-config.json");
    std::fs::write(&config_path, "not json").expect("write config");

    let md = "```mermaid\ngraph LR\n  A --> B\n```\n";
    let out = run_pandoc_in(
        None,
        "html",
        &[("GAZU_CONFIG", config_path.to_str().unwrap())],
        md,
    );
    assert!(
        !out.status.success(),
        "expected pandoc to fail on invalid config JSON"
    );

    std::fs::remove_dir_all(&dir).expect("cleanup temp dir");
}

#[test]
fn pandoc_typst_writes_svg_file_and_embeds_image() {
    pandoc_or_skip!();
    let dir = std::env::temp_dir().join(format!("gazu-it-{:?}", std::thread::current().id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");

    let md = "```mermaid\ngraph LR\n  A --> B\n```\n";
    let out = run_pandoc_in(Some(&dir), "typst", &[], md);
    assert!(
        out.status.success(),
        "pandoc failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let typst = String::from_utf8_lossy(&out.stdout);
    assert!(
        typst.contains("image("),
        "no image() in typst output: {typst}"
    );

    let svgs: Vec<_> = std::fs::read_dir(&dir)
        .expect("read temp dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".svg"))
        .collect();
    assert_eq!(
        svgs.len(),
        1,
        "expected 1 svg file in {dir:?}, found {svgs:?}"
    );

    std::fs::remove_dir_all(&dir).expect("cleanup temp dir");
}
