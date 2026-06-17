use anyhow::{ensure, Context, Result};
use sekien::{render_stream, RenderOutcome};
use serde_json::{json, Value};
use std::hash::{Hash, Hasher};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

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

    ensure!(ast["blocks"].is_array(), "no blocks in Pandoc AST");
    let mermaid_blocks = collect_mermaid_mut(&mut ast["blocks"]);

    if mermaid_blocks.is_empty() {
        print!("{input}");
        return Ok(());
    }

    let diagrams: Vec<String> = mermaid_blocks.iter().map(|b| mermaid_source(b)).collect();
    let mut outcomes = Vec::with_capacity(diagrams.len());
    render_stream(diagrams, config_json, |outcome| outcomes.push(outcome))?;

    let output_dir = (!HTML_FORMATS.contains(&format)).then_some(Path::new("gazu"));
    let plan = plan_outcomes(output_dir, outcomes);

    if let Some(dir) = output_dir {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("failed to create directory {}", dir.display()))?;
        for (path, svg) in &plan.files {
            std::fs::write(path, svg)
                .with_context(|| format!("failed to write {}", path.display()))?;
        }
    }
    apply_replacements(mermaid_blocks, plan.replacements);
    plan.warnings.iter().for_each(|w| eprintln!("{w}"));

    let out = serde_json::to_string(&ast).context("failed to serialize Pandoc AST")?;
    io::stdout()
        .write_all(out.as_bytes())
        .context("failed to write AST to stdout")?;
    Ok(())
}

/// Collects mutable references to Mermaid `CodeBlock`s anywhere in `value`,
/// in depth-first traversal order.
fn collect_mermaid_mut(value: &mut Value) -> Vec<&mut Value> {
    if is_mermaid_block(value) {
        return vec![value];
    }
    match value {
        Value::Array(items) => items.iter_mut().flat_map(collect_mermaid_mut).collect(),
        Value::Object(map) => map.values_mut().flat_map(collect_mermaid_mut).collect(),
        _ => Vec::new(),
    }
}

/// Extracts the source code from a Mermaid `CodeBlock`.
fn mermaid_source(block: &Value) -> String {
    block["c"][1].as_str().unwrap_or("").to_owned()
}

struct Plan {
    replacements: Vec<Option<Value>>,
    files: Vec<(PathBuf, String)>,
    warnings: Vec<String>,
}

/// Computes replacement values and files to write for each outcome. Pure.
///
/// - `replacements`: `Some(v)` to replace the block, `None` to leave it unchanged
/// - `files`: SVG files to write, as `(path, content)` pairs
/// - `warnings`: one entry per failed block
fn plan_outcomes(output_dir: Option<&Path>, outcomes: Vec<RenderOutcome>) -> Plan {
    let mut replacements = Vec::with_capacity(outcomes.len());
    let mut files = Vec::new();
    let mut warnings = Vec::new();
    for (i, outcome) in outcomes.into_iter().enumerate() {
        match outcome {
            RenderOutcome::Svg(svg) => {
                replacements.push(Some(match output_dir {
                    None => json!({ "t": "RawBlock", "c": ["html", svg] }),
                    Some(dir) => {
                        let (path, value) = image_block(dir, &svg);
                        files.push((path, svg));
                        value
                    }
                }));
            }
            RenderOutcome::Error(err) => {
                replacements.push(None);
                warnings.push(format!(
                    "warning: gazu: mermaid block {} failed to render:\n{err}",
                    i + 1
                ));
            }
        }
    }
    Plan {
        replacements,
        files,
        warnings,
    }
}

/// Applies replacement values to mermaid blocks in place. `None` means keep the
/// original (render failed); `Some(v)` replaces the block with `v`.
fn apply_replacements(blocks: Vec<&mut Value>, replacements: Vec<Option<Value>>) {
    for (block, replacement) in blocks.into_iter().zip(replacements) {
        if let Some(v) = replacement {
            *block = v;
        }
    }
}

/// Returns the output path and a `Para[Image]` block that references it.
/// Pure: no IO.
fn image_block(dir: &Path, svg: &str) -> (PathBuf, Value) {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    svg.hash(&mut hasher);
    let path = dir.join(format!("{:016x}.svg", hasher.finish()));
    let image_ref = path.to_string_lossy().into_owned();
    (
        path,
        json!({
            "t": "Para",
            "c": [{ "t": "Image", "c": [["", [], []], [], [image_ref, ""]] }]
        }),
    )
}

fn is_mermaid_block(block: &Value) -> bool {
    block["t"] == "CodeBlock"
        && block["c"][0][1]
            .as_array()
            .is_some_and(|cls| cls.iter().any(|c| c == "mermaid"))
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use sekien::RenderOutcome;
    use serde_json::json;

    fn mermaid(src: &str) -> Value {
        json!({ "t": "CodeBlock", "c": [["", ["mermaid"], []], src] })
    }

    fn raw(svg: &str) -> Value {
        json!({ "t": "RawBlock", "c": ["html", svg] })
    }

    fn collect_sources(root: &mut Value) -> Vec<String> {
        collect_mermaid_mut(root)
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
        let mut blocks = json!([{ "t": "Para", "c": [] }, mermaid("graph LR\n A-->B")]);
        assert_eq!(collect_sources(&mut blocks), vec!["graph LR\n A-->B"]);
    }

    #[test]
    fn collect_skips_non_mermaid() {
        let mut blocks = json!([
            { "t": "CodeBlock", "c": [["", ["rust"], []], "fn main(){}"] },
            mermaid("graph TD\n X-->Y"),
        ]);
        assert_eq!(collect_sources(&mut blocks), vec!["graph TD\n X-->Y"]);
    }

    #[test]
    fn collect_inside_div() {
        let mut blocks = json!([{
            "t": "Div",
            "c": [["", [], []], [mermaid("graph TD\n X-->Y")]]
        }]);
        assert_eq!(collect_sources(&mut blocks), vec!["graph TD\n X-->Y"]);
    }

    #[test]
    fn collect_inside_blockquote() {
        let mut blocks = json!([{
            "t": "BlockQuote",
            "c": [mermaid("graph LR\n A-->B")]
        }]);
        assert_eq!(collect_sources(&mut blocks), vec!["graph LR\n A-->B"]);
    }

    #[test]
    fn collect_inside_table_cell() {
        // Table cell: c = [Attr, Alignment, RowSpan, ColSpan, [Block]]
        let mut blocks = json!([{
            "t": "Table",
            "c": [["", [], []], null, null, null, [
                { "c": [["", [], []], "Default", 1, [{
                    "c": [["", [], []], "AlignDefault", 1, 1, [mermaid("graph LR\n A-->B")]]
                }]] }
            ]]
        }]);
        assert_eq!(collect_sources(&mut blocks), vec!["graph LR\n A-->B"]);
    }

    #[test]
    fn collect_multiple_preserves_order() {
        let mut blocks = json!([mermaid("A"), mermaid("B"), mermaid("C")]);
        assert_eq!(collect_sources(&mut blocks), vec!["A", "B", "C"]);
    }

    #[test]
    fn collect_empty() {
        let mut blocks = json!([]);
        assert!(collect_sources(&mut blocks).is_empty());
    }

    #[test]
    fn replace_rendered_substitutes_svg_for_html() {
        let outcomes = vec![RenderOutcome::Svg("<svg/>".to_owned())];
        let plan = plan_outcomes(None, outcomes);
        assert!(plan.warnings.is_empty());
        assert!(plan.files.is_empty());
        assert_eq!(plan.replacements, vec![Some(raw("<svg/>"))]);
    }

    #[test]
    fn replace_rendered_plans_image_for_non_html() {
        let dir = Path::new("gazu");
        let outcomes = vec![RenderOutcome::Svg("<svg/>".to_owned())];
        let plan = plan_outcomes(Some(dir), outcomes);
        assert!(plan.warnings.is_empty());
        assert_eq!(plan.files.len(), 1);
        let (path, svg) = &plan.files[0];
        assert!(path.to_string_lossy().ends_with(".svg"), "{path:?}");
        assert_eq!(svg, "<svg/>");
        let replacement = plan.replacements[0].as_ref().unwrap();
        assert_eq!(replacement["t"], "Para");
        assert_eq!(replacement["c"][0]["t"], "Image");
    }

    #[test]
    fn replace_failed_leaves_original_and_warns() {
        let outcomes = vec![RenderOutcome::Error("Lexical error".to_owned())];
        let plan = plan_outcomes(None, outcomes);
        assert_eq!(plan.replacements, vec![None]);
        assert!(plan.files.is_empty());
        assert_eq!(plan.warnings.len(), 1);
        assert!(
            plan.warnings[0].contains("Lexical error"),
            "{:?}",
            plan.warnings
        );
    }

    #[test]
    fn replace_inside_div() {
        let inner_mermaid = mermaid("graph LR\n A-->B");
        let mut blocks = json!([{
            "t": "Div",
            "c": [["", [], []], [inner_mermaid]]
        }]);
        let mermaid_blocks = collect_mermaid_mut(&mut blocks);
        let outcomes = vec![RenderOutcome::Svg("<svg/>".to_owned())];
        let plan = plan_outcomes(None, outcomes);
        apply_replacements(mermaid_blocks, plan.replacements);
        assert_eq!(blocks[0]["c"][1][0], raw("<svg/>"));
    }
}
