//! sekien-pandoc CLI 統合テスト。
//!
//! sekien は lib として埋め込まれているため、別途インストールは不要。
//! Linux では Xvfb が必要 (sekien が内部で起動する)。

use std::ffi::OsString;
use std::io::Write;
use std::process::{Command, Output, Stdio};

fn sekien_pandoc_bin() -> OsString {
    env!("CARGO_BIN_EXE_sekien-pandoc").into()
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

/// pandoc を `--filter sekien-pandoc` 付きで呼び出す。
fn run_pandoc(markdown: &str) -> Output {
    let mut child = Command::new("pandoc")
        .args(["-f", "markdown", "-t", "html", "--filter"])
        .arg(sekien_pandoc_bin())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn pandoc");
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
    let out = Command::new(sekien_pandoc_bin())
        .arg("--help")
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("sekien-pandoc"),
        "help missing 'sekien-pandoc': {stdout}"
    );
}

#[test]
fn version_exits_zero() {
    let out = Command::new(sekien_pandoc_bin())
        .arg("--version")
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("sekien-pandoc"),
        "--version missing 'sekien-pandoc': {stdout}"
    );
    assert!(
        stdout.contains("mermaid.js"),
        "--version missing 'mermaid.js': {stdout}"
    );
}

#[test]
fn print_lua_filter_outputs_lua() {
    let out = Command::new(sekien_pandoc_bin())
        .arg("--print-lua-filter")
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("function RawBlock"),
        "--print-lua-filter missing 'function RawBlock': {stdout}"
    );
    assert!(
        stdout.contains("svg_tmp_path"),
        "--print-lua-filter missing 'svg_tmp_path': {stdout}"
    );
}

#[test]
fn unknown_flag_is_ignored() {
    // Pandoc が output format 名を渡すケースを模倣する
    let out = Command::new(sekien_pandoc_bin())
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
