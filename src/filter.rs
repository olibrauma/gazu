use anyhow::{Context, Result, ensure};
use sekien::{RenderOutcome, render_stream};
use serde_json::{Value, json};
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
pub const HTML_FORMATS: &[&str] = &[
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

/// Returns `true` if the format allows inline HTML pass-through without external image files.
#[must_use]
pub fn is_html_format(format: &str) -> bool {
    HTML_FORMATS.contains(&format)
}

/// Helper trait allowing callers to pass Mermaid configurations in various forms:
/// `&str`, `&String`, `String`, `&Value`, `Value`, or `Option<T>`.
pub trait IntoMermaidConfig {
    /// Converts `self` into an optional serialized JSON string.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    fn into_config_json(self) -> Result<Option<String>>;
}

impl IntoMermaidConfig for &str {
    fn into_config_json(self) -> Result<Option<String>> {
        Ok(Some(self.to_owned()))
    }
}

impl IntoMermaidConfig for &String {
    fn into_config_json(self) -> Result<Option<String>> {
        Ok(Some(self.clone()))
    }
}

impl IntoMermaidConfig for String {
    fn into_config_json(self) -> Result<Option<String>> {
        Ok(Some(self))
    }
}

impl IntoMermaidConfig for &Value {
    fn into_config_json(self) -> Result<Option<String>> {
        serde_json::to_string(self)
            .map(Some)
            .context("failed to serialize Mermaid config")
    }
}

impl IntoMermaidConfig for Value {
    fn into_config_json(self) -> Result<Option<String>> {
        serde_json::to_string(&self)
            .map(Some)
            .context("failed to serialize Mermaid config")
    }
}

impl<T: IntoMermaidConfig> IntoMermaidConfig for Option<T> {
    fn into_config_json(self) -> Result<Option<String>> {
        match self {
            Some(c) => c.into_config_json(),
            None => Ok(None),
        }
    }
}

/// Determines how rendered SVG diagrams are embedded in Pandoc AST and whether files are written to disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageMode {
    /// Automatically decide based on format:
    /// - For HTML-compatible formats: inline `RawBlock("html", svg)` (no files created).
    /// - For non-HTML formats: write SVG files to the specified directory (defaults to `"gazu"`)
    ///   and embed `Image` blocks referencing them.
    Auto,

    /// Automatically decide based on format, but never write any files to disk:
    /// - For HTML-compatible formats: inline `RawBlock("html", svg)`.
    /// - For non-HTML formats: embed `Image` blocks referencing the specified directory (defaults to `"gazu"`),
    ///   and return the planned SVG files in [`FilterResult::files`] without writing to disk.
    AutoInMemory,

    /// Always embed inline `RawBlock("html", svg)`, regardless of format, with no files created.
    InlineHtml,

    /// Write SVG files to the specified directory on disk and embed `Image` blocks.
    WriteFiles(PathBuf),

    /// Embed `Image` blocks referencing the specified directory, but do NOT write files to disk.
    /// The generated SVGs and paths are returned in [`FilterResult::files`].
    InMemory(PathBuf),
}

/// Options for transforming a Pandoc AST.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterOptions<'a> {
    /// Target output format (e.g. "html", "typst", "latex", "markdown").
    pub format: &'a str,
    /// Optional serialized Mermaid configuration JSON string.
    pub config_json: Option<String>,
    /// Image embedding and file writing mode.
    pub image_mode: ImageMode,
}

impl<'a> FilterOptions<'a> {
    /// Creates options with default CLI behavior:
    /// format-dependent ([`ImageMode::Auto`]), writing files to `"gazu"` for non-HTML formats.
    #[must_use]
    pub fn new(format: &'a str) -> Self {
        Self {
            format,
            config_json: None,
            image_mode: ImageMode::Auto,
        }
    }

    /// Creates options for fully in-memory operation:
    /// never writes any files to disk ([`ImageMode::AutoInMemory`]).
    #[must_use]
    pub fn in_memory(format: &'a str) -> Self {
        Self {
            format,
            config_json: None,
            image_mode: ImageMode::AutoInMemory,
        }
    }

    /// Sets the Mermaid configuration from any supported config object or string.
    ///
    /// # Errors
    ///
    /// Returns an error if serializing `config` fails.
    pub fn with_config(mut self, config: impl IntoMermaidConfig) -> Result<Self> {
        self.config_json = config.into_config_json()?;
        Ok(self)
    }

    /// Sets the Mermaid configuration directly from an optional pre-serialized JSON string.
    #[must_use]
    pub fn with_config_json(mut self, config_json: Option<String>) -> Self {
        self.config_json = config_json;
        self
    }

    /// Sets the image mode.
    #[must_use]
    pub fn with_image_mode(mut self, mode: ImageMode) -> Self {
        self.image_mode = mode;
        self
    }

    /// Configures file writing to the specified directory for image blocks.
    #[must_use]
    pub fn with_output_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.image_mode = ImageMode::WriteFiles(dir.into());
        self
    }
}

/// Result of filtering a Pandoc AST.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FilterResult {
    /// Generated SVG files as `(path, svg_content)` pairs.
    /// If file writing was enabled ([`ImageMode::Auto`] with non-HTML or [`ImageMode::WriteFiles`]),
    /// these files were written to disk.
    /// If in-memory mode was used ([`ImageMode::AutoInMemory`] or [`ImageMode::InMemory`]),
    /// these files were NOT written to disk and are held in memory for the caller.
    pub files: Vec<(PathBuf, String)>,

    /// Warnings generated during diagram rendering (e.g. syntax errors).
    pub warnings: Vec<String>,
}

/// Result of planning diagram replacements and files. Pure.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    /// Replacement Pandoc AST values for each Mermaid block.
    pub replacements: Vec<Option<Value>>,
    /// Planned SVG files as `(path, svg_content)` pairs.
    pub files: Vec<(PathBuf, String)>,
    /// Warnings for failed blocks.
    pub warnings: Vec<String>,
}

/// Reads a Pandoc AST JSON, replaces Mermaid blocks with a representation
/// suited to `format`, and writes the result to stdout. Warnings are printed to stderr.
///
/// For non-HTML formats, SVG files are written to `gazu/`.
///
/// # Errors
///
/// Returns an error if AST parsing, diagram rendering, or I/O fails.
pub fn filter(input: &str, format: &str, config_json: Option<&str>) -> Result<()> {
    let mut ast: Value = serde_json::from_str(input).context("invalid Pandoc AST")?;

    ensure!(ast["blocks"].is_array(), "no blocks in Pandoc AST");
    let mermaid_blocks = collect_mermaid_mut(&mut ast["blocks"]);

    if mermaid_blocks.is_empty() {
        print!("{input}");
        return Ok(());
    }

    let options = FilterOptions {
        format,
        config_json: config_json.map(str::to_owned),
        image_mode: ImageMode::Auto,
    };

    let result = filter_ast_with_options(&mut ast, &options)?;
    result.warnings.iter().for_each(|w| eprintln!("{w}"));

    let out = serde_json::to_string(&ast).context("failed to serialize Pandoc AST")?;
    io::stdout()
        .write_all(out.as_bytes())
        .context("failed to write AST to stdout")?;
    Ok(())
}

/// Transforms a Pandoc AST in place according to [`FilterOptions`].
///
/// This function operates directly on the caller's AST object in memory.
/// If [`ImageMode::AutoInMemory`] or [`ImageMode::InMemory`] is configured,
/// no files are written to disk; any planned SVG files are returned in [`FilterResult::files`].
///
/// # Errors
///
/// Returns an error if the AST structure is invalid or diagram rendering fails.
pub fn filter_ast_with_options(ast: &mut Value, options: &FilterOptions) -> Result<FilterResult> {
    ensure!(ast["blocks"].is_array(), "no blocks in Pandoc AST");
    let mermaid_blocks = collect_mermaid_mut(&mut ast["blocks"]);

    if mermaid_blocks.is_empty() {
        return Ok(FilterResult::default());
    }

    let diagrams: Vec<String> = mermaid_blocks
        .iter()
        .map(|b| mermaid_source_or_empty(b).to_owned())
        .collect();
    let outcomes = render_diagrams(diagrams, options.config_json.as_deref())?;

    let (plan_dir, write_to_disk): (Option<&Path>, bool) = match &options.image_mode {
        ImageMode::Auto => {
            if is_html_format(options.format) {
                (None, false)
            } else {
                (Some(Path::new("gazu")), true)
            }
        }
        ImageMode::AutoInMemory => {
            if is_html_format(options.format) {
                (None, false)
            } else {
                (Some(Path::new("gazu")), false)
            }
        }
        ImageMode::InlineHtml => (None, false),
        ImageMode::WriteFiles(dir) => (Some(dir.as_path()), true),
        ImageMode::InMemory(dir) => (Some(dir.as_path()), false),
    };

    let plan = plan_outcomes(plan_dir, outcomes);

    if let (true, Some(dir)) = (write_to_disk, plan_dir) {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("failed to create directory {}", dir.display()))?;
        for (path, svg) in &plan.files {
            std::fs::write(path, svg)
                .with_context(|| format!("failed to write {}", path.display()))?;
        }
    }

    apply_replacements(mermaid_blocks, plan.replacements);

    Ok(FilterResult {
        files: plan.files,
        warnings: plan.warnings,
    })
}

/// Transforms a Pandoc AST in place, accepting configuration as an optional string.
///
/// # Errors
///
/// Returns an error if AST structure is invalid or rendering fails.
pub fn filter_ast(
    ast: &mut Value,
    format: &str,
    config_json: Option<&str>,
) -> Result<FilterResult> {
    let options = FilterOptions {
        format,
        config_json: config_json.map(str::to_owned),
        image_mode: ImageMode::Auto,
    };
    filter_ast_with_options(ast, &options)
}

/// Transforms a Pandoc AST in place, accepting configuration directly as a [`serde_json::Value`] object.
///
/// No configuration file is required.
///
/// # Errors
///
/// Returns an error if AST structure is invalid, serialization fails, or rendering fails.
pub fn filter_ast_value(
    ast: &mut Value,
    format: &str,
    config: Option<&Value>,
) -> Result<FilterResult> {
    let config_json = config.map(serde_json::to_string).transpose()?;
    let options = FilterOptions {
        format,
        config_json,
        image_mode: ImageMode::Auto,
    };
    filter_ast_with_options(ast, &options)
}

/// Transforms a Pandoc AST in place in-memory: never writes any files to disk.
///
/// Any generated SVGs for non-HTML formats are returned in [`FilterResult::files`].
///
/// # Errors
///
/// Returns an error if AST structure is invalid or rendering fails.
pub fn filter_ast_in_memory(
    ast: &mut Value,
    format: &str,
    config_json: Option<&str>,
) -> Result<FilterResult> {
    let options = FilterOptions {
        format,
        config_json: config_json.map(str::to_owned),
        image_mode: ImageMode::AutoInMemory,
    };
    filter_ast_with_options(ast, &options)
}

/// Transforms a Pandoc AST in place in-memory, accepting configuration directly as a [`serde_json::Value`]:
/// never writes any files to disk.
///
/// # Errors
///
/// Returns an error if AST structure is invalid, serialization fails, or rendering fails.
pub fn filter_ast_value_in_memory(
    ast: &mut Value,
    format: &str,
    config: Option<&Value>,
) -> Result<FilterResult> {
    let config_json = config.map(serde_json::to_string).transpose()?;
    let options = FilterOptions {
        format,
        config_json,
        image_mode: ImageMode::AutoInMemory,
    };
    filter_ast_with_options(ast, &options)
}

/// Filters a Pandoc AST JSON string according to [`FilterOptions`], returning the modified JSON string
/// and [`FilterResult`] without writing to stdout/stderr.
///
/// # Errors
///
/// Returns an error if parsing or rendering fails.
pub fn filter_str_with_options(
    input: &str,
    options: &FilterOptions,
) -> Result<(String, FilterResult)> {
    let mut ast: Value = serde_json::from_str(input).context("invalid Pandoc AST")?;
    let result = filter_ast_with_options(&mut ast, options)?;
    let output = serde_json::to_string(&ast).context("failed to serialize Pandoc AST")?;
    Ok((output, result))
}

/// Filters a Pandoc AST JSON string, returning the modified JSON string and [`FilterResult`].
///
/// # Errors
///
/// Returns an error if parsing or rendering fails.
pub fn filter_str(
    input: &str,
    format: &str,
    config_json: Option<&str>,
) -> Result<(String, FilterResult)> {
    let options = FilterOptions {
        format,
        config_json: config_json.map(str::to_owned),
        image_mode: ImageMode::Auto,
    };
    filter_str_with_options(input, &options)
}

/// Filters a Pandoc AST JSON string with a [`serde_json::Value`] configuration object.
///
/// # Errors
///
/// Returns an error if parsing, serialization, or rendering fails.
pub fn filter_str_value(
    input: &str,
    format: &str,
    config: Option<&Value>,
) -> Result<(String, FilterResult)> {
    let config_json = config.map(serde_json::to_string).transpose()?;
    let options = FilterOptions {
        format,
        config_json,
        image_mode: ImageMode::Auto,
    };
    filter_str_with_options(input, &options)
}

/// Renders a collection of Mermaid diagrams into SVGs.
///
/// # Errors
///
/// Returns an error if the rendering subsystem fails.
pub fn render_diagrams<I, S>(diagrams: I, config_json: Option<&str>) -> Result<Vec<RenderOutcome>>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let list: Vec<String> = diagrams.into_iter().map(Into::into).collect();
    let mut outcomes = Vec::with_capacity(list.len());
    render_stream(list, config_json, |outcome| outcomes.push(outcome))?;
    Ok(outcomes)
}

/// Renders a single Mermaid diagram into an SVG.
///
/// # Errors
///
/// Returns an error if rendering fails or produces no outcome.
pub fn render_diagram(diagram: &str, config_json: Option<&str>) -> Result<RenderOutcome> {
    let mut outcomes = render_diagrams([diagram], config_json)?;
    outcomes.pop().context("no outcome returned for diagram")
}

/// Renders a single Mermaid diagram with any supported configuration object or string.
///
/// # Errors
///
/// Returns an error if config serialization or rendering fails.
pub fn render_diagram_with_config(
    diagram: &str,
    config: impl IntoMermaidConfig,
) -> Result<RenderOutcome> {
    let config_json = config.into_config_json()?;
    render_diagram(diagram, config_json.as_deref())
}

/// Collects references to Mermaid `CodeBlock`s anywhere in `value`,
/// in depth-first traversal order.
#[must_use]
pub fn collect_mermaid(value: &Value) -> Vec<&Value> {
    if is_mermaid_block(value) {
        return vec![value];
    }
    match value {
        Value::Array(items) => items.iter().flat_map(collect_mermaid).collect(),
        Value::Object(map) => map.values().flat_map(collect_mermaid).collect(),
        _ => Vec::new(),
    }
}

/// Collects mutable references to Mermaid `CodeBlock`s anywhere in `value`,
/// in depth-first traversal order.
pub fn collect_mermaid_mut(value: &mut Value) -> Vec<&mut Value> {
    if is_mermaid_block(value) {
        return vec![value];
    }
    match value {
        Value::Array(items) => items.iter_mut().flat_map(collect_mermaid_mut).collect(),
        Value::Object(map) => map.values_mut().flat_map(collect_mermaid_mut).collect(),
        _ => Vec::new(),
    }
}

/// Extracts the source code from a Mermaid `CodeBlock`, if present.
#[must_use]
pub fn mermaid_source(block: &Value) -> Option<&str> {
    block["c"][1].as_str()
}

/// Extracts the source code from a Mermaid `CodeBlock`, or an empty string.
#[must_use]
pub fn mermaid_source_or_empty(block: &Value) -> &str {
    mermaid_source(block).unwrap_or("")
}

/// Generates a `RawBlock("html", svg)` Pandoc AST node.
#[must_use]
pub fn make_raw_html_block(svg: &str) -> Value {
    json!({ "t": "RawBlock", "c": ["html", svg] })
}

/// Generates a `Para[Image]` Pandoc AST node referencing `image_ref`.
#[must_use]
pub fn make_image_block(image_ref: &str) -> Value {
    json!({
        "t": "Para",
        "c": [{ "t": "Image", "c": [["", [], []], [], [image_ref, ""]] }]
    })
}

/// Returns the 64-bit hash of an SVG string used for generating file names.
#[must_use]
pub fn hash_svg(svg: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    svg.hash(&mut hasher);
    hasher.finish()
}

/// Returns a deterministic file name for an SVG string (e.g. `0123456789abcdef.svg`).
#[must_use]
pub fn svg_filename(svg: &str) -> String {
    format!("{:016x}.svg", hash_svg(svg))
}

/// Returns the output path and a `Para[Image]` block that references it. Pure: no IO.
#[must_use]
pub fn image_block(dir: &Path, svg: &str) -> (PathBuf, Value) {
    let path = dir.join(svg_filename(svg));
    let image_ref = path.to_string_lossy().into_owned();
    (path, make_image_block(&image_ref))
}

/// Returns `true` if `block` is a Mermaid `CodeBlock`.
#[must_use]
pub fn is_mermaid_block(block: &Value) -> bool {
    block["t"] == "CodeBlock"
        && block["c"][0][1]
            .as_array()
            .is_some_and(|cls| cls.iter().any(|c| c == "mermaid"))
}

/// Computes replacement values and files to write for each outcome. Pure.
///
/// - `replacements`: `Some(v)` to replace the block, `None` to leave it unchanged
/// - `files`: SVG files to write, as `(path, content)` pairs
/// - `warnings`: one entry per failed block
#[must_use]
pub fn plan_outcomes(output_dir: Option<&Path>, outcomes: Vec<RenderOutcome>) -> Plan {
    let mut replacements = Vec::with_capacity(outcomes.len());
    let mut files = Vec::new();
    let mut warnings = Vec::new();
    for (i, outcome) in outcomes.into_iter().enumerate() {
        match outcome {
            RenderOutcome::Svg(svg) => {
                replacements.push(Some(match output_dir {
                    None => make_raw_html_block(&svg),
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
pub fn apply_replacements(blocks: Vec<&mut Value>, replacements: Vec<Option<Value>>) {
    for (block, replacement) in blocks.into_iter().zip(replacements) {
        if let Some(v) = replacement {
            *block = v;
        }
    }
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
            .map(|b| mermaid_source_or_empty(b).to_owned())
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
    fn collect_immutable() {
        let blocks = json!([{ "t": "Para", "c": [] }, mermaid("graph LR\n A-->B")]);
        let collected = collect_mermaid(&blocks);
        assert_eq!(collected.len(), 1);
        assert_eq!(mermaid_source(collected[0]), Some("graph LR\n A-->B"));
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

    #[test]
    fn into_mermaid_config_types() {
        let str_cfg = r#"{"theme":"dark"}"#;
        assert_eq!(
            str_cfg.into_config_json().unwrap(),
            Some(str_cfg.to_owned())
        );

        let val_cfg = json!({"theme": "dark"});
        assert!((&val_cfg).into_config_json().unwrap().is_some());
        assert!(val_cfg.into_config_json().unwrap().is_some());

        let opt_none: Option<&str> = None;
        assert_eq!(opt_none.into_config_json().unwrap(), None);

        let opt_some = Some(r#"{"theme":"dark"}"#);
        assert_eq!(
            opt_some.into_config_json().unwrap(),
            Some(str_cfg.to_owned())
        );
    }

    #[test]
    fn filter_options_builder() {
        let opts = FilterOptions::new("typst")
            .with_config(json!({"theme": "dark"}))
            .unwrap()
            .with_output_dir("custom_dir");

        assert_eq!(opts.format, "typst");
        assert!(opts.config_json.unwrap().contains("dark"));
        assert_eq!(
            opts.image_mode,
            ImageMode::WriteFiles(PathBuf::from("custom_dir"))
        );
    }
}
