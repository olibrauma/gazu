use crate::renderer::{self, BlockOutcome};
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::io::{self, Write};

/// Pandoc AST JSON を受け取り、Mermaid ブロックを SVG の RawBlock に置換して stdout に書く。
pub fn filter(input: &str) -> Result<()> {
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
    let outcomes = renderer::render_blocks(diagrams)?;
    for warning in apply_outcomes(mermaid_blocks, outcomes) {
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
/// 成功したブロックは `RawBlock("html", svg)` に置換する。失敗したブロックは
/// 元の `CodeBlock` をそのまま残し、警告メッセージを返り値に積む（呼び出し側で
/// 出力する）。
fn apply_outcomes(blocks: Vec<&mut Value>, outcomes: Vec<BlockOutcome>) -> Vec<String> {
    blocks
        .into_iter()
        .zip(outcomes)
        .enumerate()
        .filter_map(|(i, (block, outcome))| match outcome {
            BlockOutcome::Rendered(svg) => {
                *block = json!({ "t": "RawBlock", "c": ["html", svg] });
                None
            }
            BlockOutcome::Failed(err) => Some(format!(
                "warning: sekien-pandoc: mermaid block {} failed to render: {err}",
                i + 1
            )),
        })
        .collect()
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
        Some("Div") => block["c"][1]
            .as_array_mut()
            .map(|v| vec![v])
            .unwrap_or_default(),
        // BlockQuote:   c = [Block]
        Some("BlockQuote") => block["c"]
            .as_array_mut()
            .map(|v| vec![v])
            .unwrap_or_default(),
        // BulletList:   c = [[Block]]
        Some("BulletList") => block["c"]
            .as_array_mut()
            .map(|items| items.iter_mut().filter_map(|i| i.as_array_mut()).collect())
            .unwrap_or_default(),
        // OrderedList:  c = [ListAttrs, [[Block]]]
        Some("OrderedList") => block["c"][1]
            .as_array_mut()
            .map(|items| items.iter_mut().filter_map(|i| i.as_array_mut()).collect())
            .unwrap_or_default(),
        _ => vec![],
    }
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
    fn replace_rendered_substitutes_svg() {
        let mut blocks = vec![mermaid("graph LR\n A-->B")];
        let mermaid_blocks = collect_mermaid_mut(&mut blocks);
        let outcomes = vec![BlockOutcome::Rendered("<svg/>".to_owned())];
        let warnings = apply_outcomes(mermaid_blocks, outcomes);
        assert!(warnings.is_empty());
        assert_eq!(blocks[0], raw("<svg/>"));
    }

    #[test]
    fn replace_failed_leaves_original_and_warns() {
        let orig = mermaid("bad diagram");
        let mut blocks = vec![orig.clone()];
        let mermaid_blocks = collect_mermaid_mut(&mut blocks);
        let outcomes = vec![BlockOutcome::Failed("Lexical error".to_owned())];
        let warnings = apply_outcomes(mermaid_blocks, outcomes);
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
        apply_outcomes(mermaid_blocks, outcomes);
        assert_eq!(blocks[0]["c"][1][0], raw("<svg/>"));
    }
}
