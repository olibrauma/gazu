# gazu — Pandoc filter for Mermaid

A Pandoc filter for Mermaid. **Fast**, **light**, **small**, and **OS-native**.

## Install

```bash
cargo install gazu
```

On Linux, building requires the WebKitGTK development packages (Ubuntu example):

```bash
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev
```

## Runtime requirements

gazu renders via an OS-native WebView.

| OS | Requirement |
|---|---|
| Linux | Xvfb (launched internally — no session or display needed) |
| macOS | Display required (WKWebView) |
| Windows | Display required (WebView2) |

Install Xvfb on Linux:

```bash
apt install xvfb                  # Debian / Ubuntu
dnf install xorg-x11-server-Xvfb  # Fedora
```

## Usage

```bash
# HTML
pandoc input.md -o output.html --filter gazu

# PDF via weasyprint
pandoc input.md -o output.pdf --pdf-engine=weasyprint --filter gazu

# PDF via typst
pandoc input.md -o output.pdf --pdf-engine=typst --filter gazu -V mainfont="Noto Sans"
```

Depending on the output format, gazu may write `gazu-<hash>.svg` files to
the current directory instead of embedding SVG inline. See
[Behavior by output format](#behavior-by-output-format) for details.

### Supported PDF engines

| PDF engine | Behavior |
|---|---|
| `weasyprint` | ✓ (via HTML) |
| `typst` | ✓ (converts SVG to an `Image` via a file) |
| `pdflatex` / `xelatex` / `lualatex` | ✗ (graphicx can't handle SVG directly; would need the `svg` package + Inkscape + `--shell-escape`) |

## CLI options

| Option | Description |
|---|---|
| `--version`, `-v` | Show version |
| `--help`, `-h` | Show help |

## Mermaid configuration (theme, fonts, etc.)

Set `GAZU_CONFIG` to a JSON file to customize `mermaid.initialize()`. Same
format as [mmdc](https://github.com/mermaid-js/mermaid-cli)'s `--configFile`:

```json
{
  "theme": "dark",
  "flowchart": { "curve": "basis" }
}
```

```bash
GAZU_CONFIG=mermaid-config.json \
  pandoc input.md -o output.html --filter gazu
```

## Behavior by output format

gazu embeds each diagram based on the output format pandoc passes it (from
`-t`/`-o`):

### Inline SVG

Formats that pass through raw HTML get `<svg>...</svg>` embedded directly,
no file created:

- HTML / slides: `html`, `html4`, `html5`, `s5`, `slidy`, `slideous`,
  `dzslides`, `revealjs`
- Markdown variants: `markdown`, `markdown_github`, `markdown_mmd`,
  `markdown_phpextra`, `markdown_strict`, `commonmark`, `commonmark_x`, `gfm`
- Others: `org`, `rst`, `mediawiki`, `muse`, `textile`, `docbook4`

### SVG file + Image

Other formats (`typst`, `latex`, etc.) drop raw HTML, so the SVG is written
to the CWD as `gazu-<hash>.svg` and embedded as an `Image`. Files remain
after conversion.

### On failure

A diagram that fails to parse or render is left as the original
` ```mermaid ` code block, with a warning on stderr.

## vs mermaid-filter

gazu is smaller, faster, and lighter than
[mermaid-filter](https://github.com/raghur/mermaid-filter):

| Metric | gazu | mermaid-filter | Advantage |
|---|---|---|---|
| Install size | **5.0 MB** | ~568 MB | **99% smaller** |
| Speed (3 diagrams) | **~2.0 s** | ~14.8 s | **~7x faster** |
| Memory (RSS) | **~446 MB** | ~849 MB | **~47% less** |

mermaid-filter spawns `mmdc` (Puppeteer/Chromium) per block; gazu renders the
whole document in one batch.

- Median of 5 runs, Linux x86_64, `util/bench/fixture.md` (3 diagrams) — see `./util/bench/bench.sh`
- Both use mermaid.js 11.14.0 (mermaid-filter 1.4.x / mmdc 11.14.0)
- Speed/Memory: filter process + children (Xvfb, WebKit, mmdc, Chromium), not pandoc itself
- Install size: gazu's binary vs. mermaid-filter's npm package + Puppeteer's Chromium download

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or https://www.apache.org/licenses/LICENSE-2.0)
- MIT License ([LICENSE-MIT](LICENSE-MIT) or https://opensource.org/licenses/MIT)

at your option.

### Bundled Assets

gazu embeds `mermaid.js` (via [sekien](https://github.com/olibrauma/sekien)).

- `mermaid.js`: Licensed under the [MIT License](mermaid.LICENSE). Copyright (c) 2014 - 2024 Knut Sveidqvist and contributors.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
