# gazu — Design

## Core architecture

**gazu is a Pandoc JSON filter.** It reads a Pandoc AST as JSON on stdin,
replaces every Mermaid `CodeBlock` with rendered output, and writes the AST
back to stdout. It runs once per `pandoc --filter gazu` invocation and exits.

All Mermaid blocks in the document are rendered in a **single batch**: gazu
collects every diagram first, then makes one call to
[sekien](https://github.com/olibrauma/sekien)'s `render_stream`, which holds
one Xvfb/WebView session open for the whole batch. Per sekien's design,
display/WebView startup dominates per-diagram render time, so paying it once
per document — instead of once per diagram — is the main reason gazu is fast.

This is the central difference from
[mermaid-filter](https://github.com/raghur/mermaid-filter), which spawns
`mmdc` (a Puppeteer/Chromium process) once per diagram. See
[README — vs mermaid-filter](../../README.md#vs-mermaid-filter) for measurements.

## Internals

### Pipeline (`filter()`, in `src/pandoc.rs`)

1. Parse the input as a `serde_json::Value` (no typed Pandoc AST struct — the
   AST is large and gazu only touches a handful of node shapes).
2. `collect_mermaid_mut`: find every Mermaid `CodeBlock` anywhere in the AST,
   as `&mut Value`.
3. If none, print the input unchanged and exit (no sekien call, no Xvfb).
4. Extract each block's source and pass them to `renderer::render_blocks`,
   which wraps `sekien::render_stream`.
5. `apply_outcomes`: for each block, in place, either replace it with the
   rendered SVG (`RawBlock` or `Image`, depending on `format`) or leave it
   untouched and emit a warning.
6. Serialize and write the AST to stdout.

### AST traversal

`collect_mermaid_mut` recurses into every `Value::Array` and `Value::Object`
uniformly:

```rust
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
```

There is no list of "container" block types to recurse into — every node is
visited. This is safe because a `CodeBlock` only ever appears in `Block`
position (Pandoc has no inline code-block variant), so a node that happens to
match `is_mermaid_block`'s shape (`{"t":"CodeBlock","c":[Attr, String]}` with
a `mermaid` class) is, in practice, always a real Mermaid block.

### Inline SVG vs. file + `Image`

`apply_outcomes` decides per output format (`format`, Pandoc's `-t`/`-o`
target) whether a rendered block becomes:

- `RawBlock("html", "<svg>...</svg>")` — for formats in `HTML_FORMATS`
  (`src/pandoc.rs`), which pass raw HTML through (or translate it to the
  format's own raw-HTML syntax).
- `Para[Image]` pointing at `gazu/<hash>.svg`, written to a `gazu/`
  subdirectory of the current directory — for everything else (`typst`,
  `latex`, ...), which drop raw HTML.

`HTML_FORMATS` is not a guess: it's measured against pandoc 3.7 by
`util/check-html-formats.sh`, which runs a `RawBlock("html", ...)` AST
through every writer and checks whether the HTML survives. Re-run it when
bumping the pandoc version.

The SVG file can't be deleted by gazu — pandoc reads it during its own
generation phase, after the filter has already exited and written its output.

### Image filenames and output directory

Generated SVG files are written to a `gazu/` subdirectory of pandoc's CWD
(created on first use if absent), with each file named `<hash>.svg`.

**Why a subdirectory?** gazu cannot delete its own output — pandoc reads the
SVG files during its own generation phase, after the filter has already
exited. Scattering files directly in the CWD makes cleanup a glob operation
(`rm gazu-*.svg`) that is easy to overlook. Grouping them under `gazu/`
makes removal unambiguous and one command (`rm -rf gazu/`).

**Why not a hidden directory (`.gazu/`)?** Because gazu requires the user to
perform cleanup manually, hiding the directory would obscure the fact that
files were left behind. A visible `gazu/` directory is a clear signal that
generated artifacts exist and need to be managed.

The hash uses `DefaultHasher` (SipHash) over the SVG content. It only needs
to (a) avoid collisions between distinct diagrams in one document and (b) let
pandoc find the file by the path gazu wrote into the `Image` node — both
within a single process run. `DefaultHasher`'s lack of a cross-version
stability guarantee is irrelevant for this use.

### Failure handling

A Mermaid block that fails to parse/render is left as the original
` ```mermaid ` `CodeBlock` — unchanged in the output document — and a warning
is printed to stderr. The rest of the document, including other Mermaid
blocks, is processed normally. This mirrors sekien's per-block
`RenderOutcome::Error` / continue-on-error model: one bad diagram doesn't
fail the whole `pandoc` invocation.

### Mermaid configuration

`GAZU_CONFIG` (env var) points at a JSON file, read by `load_config_json` in
`src/main.rs` and passed through to `render_stream` as `config_json`. The
format is exactly mmdc's `--configFile` format (an object merged into
`mermaid.initialize()`) — gazu defines no config format of its own.

## Why this design

### Why one batched `render_stream` call, not one process per diagram

mermaid-filter's per-diagram `mmdc` spawn re-pays Puppeteer/Chromium startup
for every diagram. gazu's batching amortizes Xvfb + WebView + mermaid.js load
over the whole document — the gap widens with diagram count. See
[util/bench](../bench).

### Why a generic traversal instead of a container whitelist

The previous implementation recursed only into a fixed set of "container"
block types (`Div`, `BlockQuote`, `BulletList`, `OrderedList`). This had two
problems:

- It encoded a "this container is processed / this one is ignored"
  distinction that has no relation to user intent — a Mermaid code block
  inside, say, a `Table` cell or `Figure` was silently left as a raw code
  block, for no reason the user could anticipate.
- The whitelist would need to grow indefinitely as Pandoc's AST gains new
  container node types.

Recursing into every `Array`/`Object` removes both: any Mermaid `CodeBlock`,
wherever it's nested, is rendered, and gazu carries no per-node-type logic at
all. `collect_inside_table_cell` (in `src/pandoc.rs`'s tests) exercises a case
— a Mermaid block in a Table cell — that the old whitelist did not handle.

### Why no env var to force inline-SVG vs. file output

Considered and rejected. The `HTML_FORMATS`-based choice is the only
combination empirically known to work per writer; a manual override would let
a user select a combination that's silently broken (e.g. forcing inline SVG
into `latex`, where pandoc drops raw HTML entirely). Keeping the choice
automatic also matches gazu's no-configuration-needed philosophy — see
[Mermaid configuration](#mermaid-configuration) for the one knob gazu does
expose.

### Why LaTeX-based `--pdf-engine`s aren't supported

Investigated directly (not assumed): LaTeX writers drop `RawBlock("html",
...)` entirely, so inline SVG is silently lost. The file + `Image` path also
fails unless the `svg` LaTeX package, `--shell-escape`, and `rsvg-convert` or
`inkscape` are all available — and even then it's extra setup outside gazu's
control. Rather than special-case or auto-configure LaTeX, the README points
users at `--pdf-engine=weasyprint` or `=typst`, both of which work with no
extra setup.

### Why `"html"` is the default format when none is provided

Pandoc always passes the output format as a positional argument when invoking
a filter, so `resolve_command()` receiving no positional argument means gazu
was invoked directly by the user, not by pandoc. If stdin is a TTY in that
case, `Command::Help` is returned. If stdin is a pipe (e.g. manual testing
with `echo '...' | gazu`), `Command::Filter("html")` is returned as a
fallback.

`"html"` was considered for `Option<String>` to make "no format specified"
explicit in the type. Rejected: `None` would still need to be treated as
`"html"` inside `filter()`, so the `Option` layer would encode a distinction
that has no effect on behaviour — the type would be lying. A plain `"html"`
default is honest and produces the least surprising behaviour (inline SVG, no
files written).

### Why no PNG output

Raised and rejected. Converting SVG to PNG would add a real feature surface
(an image format converter) for a use case with no concrete need, and SVG
already embeds cleanly in every supported path. Out of scope for "Fast,
light, small, and OS-native".
