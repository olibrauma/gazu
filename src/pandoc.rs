use crate::renderer::{self, BlockOutcome};
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::hash::{Hash, Hasher};
use std::io::{self, Write};
use std::path::Path;

/// Formats that pass `RawBlock("html", svg)` through as-is (or translate it
/// into the format's own raw-HTML syntax). For all other formats (typst,
/// latex, etc.) the SVG is written to a file and converted to an `Image`.
///
/// Measured against pandoc 3.7 with `util/check-html-formats.sh`. Re-run
/// that script and revisit this list when bumping the pandoc version.
/// `chunkedhtml` is excluded because it's a multi-file writer (requires
/// `-o <directory>`) and can't be checked by that script.
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

/// Reads a Pandoc AST JSON, replaces Mermaid blocks with a representation
/// suited to `format`, and writes the result to stdout.
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

/// Collects mutable references to Mermaid `CodeBlock`s in depth-first
/// traversal order (also descends into Div, BlockQuote, and list blocks).
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

/// Extracts the source code from a Mermaid `CodeBlock`.
fn mermaid_source(block: &Value) -> String {
    block["c"][1].as_str().unwrap_or("").to_owned()
}

/// Applies `outcomes` 1:1 to the blocks collected by `collect_mermaid_mut`.
///
/// A successful block becomes `RawBlock("html", svg)` if `format` passes raw
/// HTML through, or otherwise has its SVG written to a file under `dir` and
/// is replaced with `Para[Image]`. A failed block is left as the original
/// `CodeBlock`, and a warning message is pushed onto the returned `Vec`
/// (the caller prints it).
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

/// Writes `svg` to a file under `dir` and returns a `Para[Image]` block that
/// references it.
///
/// Output formats that don't pass raw HTML through (typst, latex, etc.)
/// can't embed SVG inline, so it's converted to an `Image` node backed by a
/// file. `dir` must match pandoc's own current directory, since formats like
/// typst resolve the file path relative to their root (= pandoc's CWD).
/// The file can't be deleted here, since pandoc reads it during its
/// generation phase, after the filter has exited.
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

/// Returns mutable references to a container block's child block lists.
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

/// Returns mutable references to `[[Block]]` (the block list of each list item).
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
