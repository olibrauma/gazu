use crate::renderer::{self, BlockOutcome};
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::hash::{Hash, Hasher};
use std::io::{self, Write};
use std::path::Path;

/// `RawBlock("html", svg)` を素通り (またはフォーマット独自の raw-html 構文に
/// 変換) して出力するフォーマット。これら以外 (typst, latex 等) では SVG を
/// ファイルに書き出し `Image` に変換する。
///
/// `util/check-html-formats.sh` の実測結果 (pandoc 3.7)。pandoc のバージョンを
/// 上げたときはこのスクリプトを再実行し、リストを見直すこと。
/// `chunkedhtml` はマルチファイル writer (`-o <directory>` 必須) のため
/// このスクリプトでは検証できず対象外。
const HTML_FORMATS: &[&str] = &[
    "html",
    "html4",
    "html5",
    "s5",
    "slidy",
    "slideous",
    "dzslides",
    "revealjs",
    "markdown",
    "markdown_github",
    "markdown_mmd",
    "markdown_phpextra",
    "markdown_strict",
    "commonmark",
    "commonmark_x",
    "gfm",
    "org",
    "rst",
    "mediawiki",
    "muse",
    "textile",
    "docbook4",
];

/// Pandoc AST JSON を受け取り、Mermaid ブロックを `format` に応じた表現に置換して stdout に書く。
pub fn filter(input: &str, format: &str, config_json: Option<&str>) -> Result<()> {
    let mut ast: Value = serde_json::from_str(input).context("invalid Pandoc AST")?;

    let blocks_mut = ast["blocks"]
        .as_array_mut()
        .context("no blocks in Pandoc AST")?;
    let mermaid_blocks = collect_mermaid_mut(blocks_mut);

    if mermaid_blocks.is_empty() {
        print!("{input}");
        return Ok(());
    }

    let diagrams = mermaid_blocks.iter().map(|b| mermaid_source(b)).collect();
    let outcomes = renderer::render_blocks(diagrams, config_json)?;
    for warning in apply_outcomes(Path::new("."), format, mermaid_blocks, outcomes)? {
        eprintln!("{warning}");
    }

    let out = serde_json::to_string(&ast).context("failed to serialize Pandoc AST")?;
    io::stdout()
        .write_all(out.as_bytes())
        .context("failed to write AST to stdout")?;
    Ok(())
}

/// Mermaid CodeBlock への可変参照を深さ優先の走査順で収集する
/// （Div・BlockQuote・リスト内のブロックも対象）。
fn collect_mermaid_mut(blocks: &mut [Value]) -> Vec<&mut Value> {
    let mut out = Vec::new();
    for block in blocks.iter_mut() {
        if is_mermaid_block(block) {
            out.push(block);
        } else {
            for inner in nested_mut(block) {
                out.extend(collect_mermaid_mut(inner));
            }
        }
    }
    out
}

/// Mermaid CodeBlock のソースコードを取り出す。
fn mermaid_source(block: &Value) -> String {
    block["c"][1].as_str().unwrap_or("").to_owned()
}

/// `collect_mermaid_mut` で収集したブロックに `outcomes` を `1:1` で適用する。
///
/// 成功したブロックは `format` が raw HTML を素通りするものなら
/// `RawBlock("html", svg)` に、それ以外 (typst, latex 等) なら SVG を `dir`
/// にファイルとして書き出し `Para[Image]` に置換する。失敗したブロックは
/// 元の `CodeBlock` をそのまま残し、警告メッセージを返り値に積む（呼び出し側で
/// 出力する）。
fn apply_outcomes(
    dir: &Path,
    format: &str,
    blocks: Vec<&mut Value>,
    outcomes: Vec<BlockOutcome>,
) -> Result<Vec<String>> {
    let raw_html_ok = HTML_FORMATS.contains(&format);
    let mut warnings = Vec::new();
    for (i, (block, outcome)) in blocks.into_iter().zip(outcomes).enumerate() {
        match outcome {
            BlockOutcome::Rendered(svg) => {
                *block = if raw_html_ok {
                    json!({ "t": "RawBlock", "c": ["html", svg] })
                } else {
                    image_block(dir, &svg)?
                };
            }
            BlockOutcome::Failed(err) => warnings.push(format!(
                "warning: gazu: mermaid block {} failed to render: {err}",
                i + 1
            )),
        }
    }
    Ok(warnings)
}

/// `svg` を `dir` にファイルとして書き出し、それを参照する `Para[Image]`
/// ブロックを返す。
///
/// raw HTML を素通りしない出力フォーマット (typst, latex 等) では SVG を
/// インライン埋め込みできないため、ファイル経由の `Image` ノードに変換する。
/// `dir` は呼び出し元 (pandoc) のカレントディレクトリと一致させる必要がある
/// （typst 等はファイルパスを自身の root = pandoc の CWD 基準で解決するため）。
/// pandoc は filter 終了後の生成フェーズでファイルを読むため、ここで削除はできない。
fn image_block(dir: &Path, svg: &str) -> Result<Value> {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    svg.hash(&mut hasher);
    let filename = format!("gazu-{:016x}.svg", hasher.finish());

    std::fs::write(dir.join(&filename), svg)
        .with_context(|| format!("failed to write {filename}"))?;

    Ok(json!({
        "t": "Para",
        "c": [{ "t": "Image", "c": [["", [], []], [], [filename, ""]] }]
    }))
}

fn is_mermaid_block(block: &Value) -> bool {
    block["t"] == "CodeBlock"
        && block["c"][0][1]
            .as_array()
            .is_some_and(|cls| cls.iter().any(|c| c == "mermaid"))
}

/// コンテナブロックの子ブロック列を可変参照として返す。
fn nested_mut(block: &mut Value) -> Vec<&mut Vec<Value>> {
    match block["t"].as_str() {
        // Div:          c = [Attr, [Block]]
        Some("Div") => block["c"][1].as_array_mut().into_iter().collect(),
        // BlockQuote:   c = [Block]
        Some("BlockQuote") => block["c"].as_array_mut().into_iter().collect(),
        // BulletList:   c = [[Block]]
        Some("BulletList") => list_items_mut(block["c"].as_array_mut()),
        // OrderedList:  c = [ListAttrs, [[Block]]]
        Some("OrderedList") => list_items_mut(block["c"][1].as_array_mut()),
        _ => vec![],
    }
}

/// `[[Block]]` (リスト各アイテムのブロック列) を可変参照として返す。
fn list_items_mut(items: Option<&mut Vec<Value>>) -> Vec<&mut Vec<Value>> {
    items
        .into_iter()
        .flat_map(|items| items.iter_mut().filter_map(|i| i.as_array_mut()))
        .collect()
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn mermaid(src: &str) -> Value {
        json!({ "t": "CodeBlock", "c": [["", ["mermaid"], []], src] })
    }

    fn raw(svg: &str) -> Value {
        json!({ "t": "RawBlock", "c": ["html", svg] })
    }

    fn collect_sources(blocks: &mut [Value]) -> Vec<String> {
        collect_mermaid_mut(blocks)
            .into_iter()
            .map(|b| mermaid_source(b))
            .collect()
    }

    #[test]
    fn is_mermaid_true() {
        assert!(is_mermaid_block(&mermaid("graph LR\n A-->B")));
    }

    #[test]
    fn is_mermaid_false_wrong_class() {
        let block = json!({ "t": "CodeBlock", "c": [["", ["rust"], []], "fn main(){}"] });
        assert!(!is_mermaid_block(&block));
    }

    #[test]
    fn is_mermaid_false_non_codeblock() {
        assert!(!is_mermaid_block(&json!({ "t": "Para", "c": [] })));
    }

    #[test]
    fn collect_top_level() {
        let mut blocks = vec![json!({ "t": "Para", "c": [] }), mermaid("graph LR\n A-->B")];
        assert_eq!(collect_sources(&mut blocks), vec!["graph LR\n A-->B"]);
    }

    #[test]
    fn collect_skips_non_mermaid() {
        let mut blocks = vec![
            json!({ "t": "CodeBlock", "c": [["", ["rust"], []], "fn main(){}"] }),
            mermaid("graph TD\n X-->Y"),
        ];
        assert_eq!(collect_sources(&mut blocks), vec!["graph TD\n X-->Y"]);
    }

    #[test]
    fn collect_inside_div() {
        let div = json!({
            "t": "Div",
            "c": [["", [], []], [mermaid("graph TD\n X-->Y")]]
        });
        assert_eq!(collect_sources(&mut [div]), vec!["graph TD\n X-->Y"]);
    }

    #[test]
    fn collect_inside_blockquote() {
        let bq = json!({
            "t": "BlockQuote",
            "c": [mermaid("graph LR\n A-->B")]
        });
        assert_eq!(collect_sources(&mut [bq]), vec!["graph LR\n A-->B"]);
    }

    #[test]
    fn collect_multiple_preserves_order() {
        let mut blocks = vec![mermaid("A"), mermaid("B"), mermaid("C")];
        assert_eq!(collect_sources(&mut blocks), vec!["A", "B", "C"]);
    }

    #[test]
    fn collect_empty() {
        assert!(collect_sources(&mut []).is_empty());
    }

    #[test]
    fn replace_rendered_substitutes_svg_for_html() {
        let mut blocks = vec![mermaid("graph LR\n A-->B")];
        let mermaid_blocks = collect_mermaid_mut(&mut blocks);
        let outcomes = vec![BlockOutcome::Rendered("<svg/>".to_owned())];
        let warnings = apply_outcomes(Path::new("."), "html", mermaid_blocks, outcomes).unwrap();
        assert!(warnings.is_empty());
        assert_eq!(blocks[0], raw("<svg/>"));
    }

    #[test]
    fn replace_rendered_writes_image_for_non_html() {
        let dir = std::env::temp_dir().join(format!("gazu-test-{:?}", std::thread::current().id()));
        std::fs::create_dir_all(&dir).unwrap();

        let mut blocks = vec![mermaid("graph LR\n A-->B")];
        let mermaid_blocks = collect_mermaid_mut(&mut blocks);
        let outcomes = vec![BlockOutcome::Rendered("<svg/>".to_owned())];
        let warnings = apply_outcomes(&dir, "typst", mermaid_blocks, outcomes).unwrap();
        assert!(warnings.is_empty());

        assert_eq!(blocks[0]["t"], "Para");
        let image = &blocks[0]["c"][0];
        assert_eq!(image["t"], "Image");
        let filename = image["c"][2][0].as_str().unwrap();
        assert!(
            filename.starts_with("gazu-") && filename.ends_with(".svg"),
            "{filename}"
        );
        assert_eq!(
            std::fs::read_to_string(dir.join(filename)).unwrap(),
            "<svg/>"
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn replace_failed_leaves_original_and_warns() {
        let orig = mermaid("bad diagram");
        let mut blocks = vec![orig.clone()];
        let mermaid_blocks = collect_mermaid_mut(&mut blocks);
        let outcomes = vec![BlockOutcome::Failed("Lexical error".to_owned())];
        let warnings = apply_outcomes(Path::new("."), "html", mermaid_blocks, outcomes).unwrap();
        assert_eq!(blocks[0], orig);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("Lexical error"), "{warnings:?}");
    }

    #[test]
    fn replace_inside_div() {
        let inner_mermaid = mermaid("graph LR\n A-->B");
        let mut blocks = vec![json!({
            "t": "Div",
            "c": [["", [], []], [inner_mermaid]]
        })];
        let mermaid_blocks = collect_mermaid_mut(&mut blocks);
        let outcomes = vec![BlockOutcome::Rendered("<svg/>".to_owned())];
        apply_outcomes(Path::new("."), "html", mermaid_blocks, outcomes).unwrap();
        assert_eq!(blocks[0]["c"][1][0], raw("<svg/>"));
    }
}
