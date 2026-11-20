use gazu::{
    FilterOptions, ImageMode, RenderOutcome, collect_mermaid, collect_mermaid_mut, filter_ast,
    filter_ast_in_memory, filter_ast_value, filter_ast_value_in_memory, filter_ast_with_options,
    filter_str, filter_str_value, is_html_format, is_mermaid_block, mermaid_source,
    mermaid_source_or_empty, mermaid_version, render_diagram, render_diagram_with_config,
    render_diagrams, svg_filename, usage, version,
};
use serde_json::{Value, json};
use std::path::Path;

fn sample_ast_with_mermaid(diagram: &str) -> Value {
    json!({
        "pandoc-api-version": [1, 23],
        "meta": {},
        "blocks": [
            {
                "t": "Header",
                "c": [1, ["intro", [], []], [{ "t": "Str", "c": "Diagram" }]]
            },
            {
                "t": "CodeBlock",
                "c": [["", ["mermaid"], []], diagram]
            }
        ]
    })
}

fn test_versions_and_usage() {
    print!("test test_versions_and_usage ... ");
    assert!(!version().is_empty());
    assert!(!mermaid_version().is_empty());
    assert!(usage().contains("gazu"));
    println!("ok");
}

fn test_format_check() {
    print!("test test_format_check ... ");
    assert!(is_html_format("html"));
    assert!(is_html_format("revealjs"));
    assert!(is_html_format("gfm"));
    assert!(!is_html_format("typst"));
    assert!(!is_html_format("latex"));
    assert!(!is_html_format("pdf"));
    println!("ok");
}

fn test_render_diagram_directly() {
    print!("test test_render_diagram_directly ... ");
    let outcome = render_diagram("graph LR\n  A --> B", None).expect("render should succeed");
    match outcome {
        RenderOutcome::Svg(svg) => {
            assert!(svg.contains("<svg"), "expected SVG tag: {svg}");
        }
        RenderOutcome::Error(err) => panic!("render unexpectedly failed: {err}"),
    }
    println!("ok");
}

fn test_render_diagram_with_config_value() {
    print!("test test_render_diagram_with_config_value ... ");
    let config = json!({ "theme": "dark" });
    let outcome =
        render_diagram_with_config("graph LR\n  A --> B", &config).expect("render should succeed");
    match outcome {
        RenderOutcome::Svg(svg) => {
            assert!(svg.contains("<svg"), "expected SVG tag: {svg}");
            assert!(svg.contains("#1f2020"), "expected dark theme: {svg}");
        }
        RenderOutcome::Error(err) => panic!("render unexpectedly failed: {err}"),
    }
    println!("ok");
}

fn test_render_diagrams_batch() {
    print!("test test_render_diagrams_batch ... ");
    let diagrams = ["graph LR\n  A --> B", "graph TD\n  C --> D"];
    let outcomes = render_diagrams(diagrams, None).expect("batch render");
    assert_eq!(outcomes.len(), 2);
    for outcome in outcomes {
        assert!(matches!(outcome, RenderOutcome::Svg(_)));
    }
    println!("ok");
}

fn test_render_invalid_diagram_returns_error_outcome() {
    print!("test test_render_invalid_diagram_returns_error_outcome ... ");
    let outcome = render_diagram("invalidMermaidDiagramSyntax@@##", None).expect("render call");
    assert!(matches!(outcome, RenderOutcome::Error(_)));
    println!("ok");
}

fn test_filter_ast_html_inlines_svg() {
    print!("test test_filter_ast_html_inlines_svg ... ");
    let mut ast = sample_ast_with_mermaid("graph LR\n  A --> B");
    let result = filter_ast(&mut ast, "html", None).expect("filter AST");

    assert!(result.warnings.is_empty());
    assert!(result.files.is_empty());

    let second_block = &ast["blocks"][1];
    assert_eq!(second_block["t"], "RawBlock");
    assert_eq!(second_block["c"][0], "html");
    let svg = second_block["c"][1].as_str().expect("svg string");
    assert!(svg.contains("<svg"));
    println!("ok");
}

fn test_filter_ast_with_config_value_no_files() {
    print!("test test_filter_ast_with_config_value_no_files ... ");
    let mut ast = sample_ast_with_mermaid("graph LR\n  A --> B");
    let config = json!({ "theme": "dark" });

    // Directly pass the serde_json::Value object without any file
    let result = filter_ast_value(&mut ast, "html", Some(&config)).expect("filter AST with value");

    assert!(result.warnings.is_empty());
    assert!(result.files.is_empty());

    let svg = ast["blocks"][1]["c"][1].as_str().unwrap();
    assert!(
        svg.contains("#1f2020"),
        "dark theme color should be present"
    );
    println!("ok");
}

fn test_filter_ast_in_memory_non_html_creates_no_files_on_disk() {
    print!("test test_filter_ast_in_memory_non_html_creates_no_files_on_disk ... ");
    let mut ast = sample_ast_with_mermaid("graph LR\n  A --> B");

    // "typst" normally writes files to gazu/, but in-memory mode never touches disk
    let result = filter_ast_in_memory(&mut ast, "typst", None).expect("filter in memory");

    assert!(result.warnings.is_empty());
    assert_eq!(
        result.files.len(),
        1,
        "expected 1 in-memory generated SVG file"
    );

    let (path, svg_content) = &result.files[0];
    assert!(path.to_string_lossy().contains("gazu"));
    assert!(svg_content.contains("<svg"));

    // Ensure the AST references the image block
    let second_block = &ast["blocks"][1];
    assert_eq!(second_block["t"], "Para");
    assert_eq!(second_block["c"][0]["t"], "Image");

    let disk_file = Path::new(&path);
    if disk_file.exists() {
        let _ = std::fs::remove_file(disk_file);
    }

    // Run again to verify it strictly doesn't write
    let mut ast2 = sample_ast_with_mermaid("graph LR\n  A --> B");
    let result2 =
        filter_ast_value_in_memory(&mut ast2, "typst", None).expect("filter in memory again");
    let (path2, _) = &result2.files[0];
    assert!(
        !path2.exists(),
        "in-memory mode must NOT create file on disk: {path2:?}"
    );
    println!("ok");
}

fn test_filter_ast_with_options_custom_output_dir() {
    print!("test test_filter_ast_with_options_custom_output_dir ... ");
    let temp_dir =
        std::env::temp_dir().join(format!("gazu-lib-test-{:?}", std::thread::current().id()));
    let _ = std::fs::remove_dir_all(&temp_dir);

    let mut ast = sample_ast_with_mermaid("graph LR\n  A --> B");
    let options = FilterOptions::new("typst").with_output_dir(&temp_dir);

    let result = filter_ast_with_options(&mut ast, &options).expect("filter with custom dir");
    assert_eq!(result.files.len(), 1);

    let (path, _) = &result.files[0];
    assert!(path.starts_with(&temp_dir));
    assert!(
        path.exists(),
        "custom output dir file should be written to disk"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
    println!("ok");
}

fn test_filter_ast_with_options_inline_html_mode() {
    print!("test test_filter_ast_with_options_inline_html_mode ... ");
    let mut ast = sample_ast_with_mermaid("graph LR\n  A --> B");
    // Force inline HTML mode on non-HTML format typst
    let options = FilterOptions::new("typst").with_image_mode(ImageMode::InlineHtml);

    let result = filter_ast_with_options(&mut ast, &options).expect("filter inline mode");
    assert!(result.files.is_empty());
    assert_eq!(ast["blocks"][1]["t"], "RawBlock");
    println!("ok");
}

fn test_filter_str_and_filter_str_value() {
    print!("test test_filter_str_and_filter_str_value ... ");
    let input = serde_json::to_string(&sample_ast_with_mermaid("graph LR\n  A --> B")).unwrap();
    let config = json!({ "theme": "dark" });

    let (output, result) =
        filter_str_value(&input, "html", Some(&config)).expect("filter_str_value");
    assert!(result.warnings.is_empty());
    assert!(output.contains("<svg"));
    assert!(output.contains("#1f2020"));

    // Also verify plain filter_str
    let (output2, _) = filter_str(&input, "html", None).expect("filter_str");
    assert!(output2.contains("<svg"));
    println!("ok");
}

fn test_inspect_and_manipulate_blocks() {
    print!("test test_inspect_and_manipulate_blocks ... ");
    let mut ast = sample_ast_with_mermaid("graph TD\n  Node1 --> Node2");

    let blocks = collect_mermaid(&ast);
    assert_eq!(blocks.len(), 1);
    assert!(is_mermaid_block(blocks[0]));
    assert_eq!(
        mermaid_source(blocks[0]),
        Some("graph TD\n  Node1 --> Node2")
    );
    assert_eq!(
        mermaid_source_or_empty(blocks[0]),
        "graph TD\n  Node1 --> Node2"
    );

    let non_mermaid = json!({ "t": "Para", "c": [] });
    assert!(!is_mermaid_block(&non_mermaid));
    assert_eq!(mermaid_source(&non_mermaid), None);
    assert_eq!(mermaid_source_or_empty(&non_mermaid), "");

    let mut blocks_mut = collect_mermaid_mut(&mut ast);
    assert_eq!(blocks_mut.len(), 1);
    blocks_mut[0]["c"][1] = json!("graph LR\n  Edited --> Node");

    assert_eq!(
        mermaid_source(blocks_mut[0]),
        Some("graph LR\n  Edited --> Node")
    );
    println!("ok");
}

fn test_svg_filename_helper() {
    print!("test test_svg_filename_helper ... ");
    let svg = "<svg viewBox='0 0 100 100'></svg>";
    let filename = svg_filename(svg);
    assert!(filename.ends_with(".svg"));
    assert_eq!(filename.len(), 16 + 4); // 16 hex chars + .svg
    println!("ok");
}

fn main() {
    println!("\nrunning library integration tests (main thread):");
    test_versions_and_usage();
    test_format_check();
    test_render_diagram_directly();
    test_render_diagram_with_config_value();
    test_render_diagrams_batch();
    test_render_invalid_diagram_returns_error_outcome();
    test_filter_ast_html_inlines_svg();
    test_filter_ast_with_config_value_no_files();
    test_filter_ast_in_memory_non_html_creates_no_files_on_disk();
    test_filter_ast_with_options_custom_output_dir();
    test_filter_ast_with_options_inline_html_mode();
    test_filter_str_and_filter_str_value();
    test_inspect_and_manipulate_blocks();
    test_svg_filename_helper();
    println!("test result: ok. 14 passed; 0 failed.\n");
}
