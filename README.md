# sekien-pandoc — Pandoc filter for Mermaid

Pandoc 文書中の Mermaid コードブロックを SVG に変換するフィルタ。
[sekien](https://github.com/olibrauma/sekien) を library としてリンクし、
文書内の全ブロックを 1 回の起動でまとめてレンダリングする。

## 前提

Linux では sekien が起動時に内部で Xvfb を立ち上げるため、xvfb が必要:

```bash
apt install xvfb       # Debian / Ubuntu
dnf install xorg-x11-server-Xvfb  # Fedora
```

ビルド時は WebKitGTK の開発パッケージも必要 (Ubuntu の例):

```bash
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev
```

macOS / Windows ではディスプレイ接続が必要（sekien が OS ネイティブ WebView を
呼ぶため）。

## インストール

```bash
cargo install sekien-pandoc
```

## 使い方

### HTML 出力

```bash
pandoc input.md -o output.html --filter sekien-pandoc
```

文書中の Mermaid コードブロック（` ```mermaid ` で始まるもの）を
`<svg>` に置換して AST に埋め込む。Mermaid の解析に失敗したブロックは
元の `CodeBlock` をそのまま残し（graceful fallback）、stderr に warning を出す。

### PDF 出力

#### weasyprint (HTML 経由)

```bash
pandoc input.md -o output.pdf \
  --pdf-engine=weasyprint \
  --filter sekien-pandoc
```

#### typst / pdflatex など (Lua filter が必要)

typst・pdflatex 等、raw HTML を drop する PDF engine では、SVG を一旦ファイルに
書き出して `Image` ノードに変換する同梱の Lua filter を組み合わせる。

```bash
pandoc input.md -o output.pdf \
  --pdf-engine=typst \
  --pdf-engine-opt=--root=/ \
  --filter sekien-pandoc \
  --lua-filter <(sekien-pandoc --print-lua-filter) \
  -V mainfont="Noto Sans"
```

`--pdf-engine-opt=--root=/` は typst のファイルシステムルートを `/` にする
オプション。これにより Lua filter が `/tmp/` へ書き出した一時 SVG ファイルを
typst が参照できるようになり、PDF 生成後は OS が自動的に掃除する。

`<(...)` が使えない環境では先にファイルへ書き出す:

```bash
sekien-pandoc --print-lua-filter > sekien.lua

pandoc input.md -o output.pdf \
  --pdf-engine=typst \
  --pdf-engine-opt=--root=/ \
  --filter sekien-pandoc \
  --lua-filter sekien.lua \
  -V mainfont="Noto Sans"
```

常用するなら pandoc の user data directory に置くとパスなしで参照できる:

```bash
sekien-pandoc --print-lua-filter \
  > "$(pandoc --version | awk '/User data/{print $4}')/filters/sekien.lua"

pandoc input.md -o output.pdf \
  --pdf-engine=typst \
  --pdf-engine-opt=--root=/ \
  --filter sekien-pandoc \
  --lua-filter sekien.lua \
  -V mainfont="Noto Sans"
```

#### 対応 PDF engine

| PDF engine | 動作 |
|---|---|
| `weasyprint` | ✓ (HTML 経由、Lua filter 不要) |
| `typst` | ✓ (Lua filter + `--pdf-engine-opt=--root=/` 必要) |
| `pdflatex` / `xelatex` / `lualatex` | ✗ (raw HTML を drop、SVG package 別途必要) |

## CLI オプション

| オプション | 説明 |
|---|---|
| `--print-lua-filter` | 同梱の Lua filter を stdout に出力する |
| `--version`, `-v` | バージョンを表示する |
| `--help`, `-h` | ヘルプを表示する |

## 動作概要

1. stdin から Pandoc AST (JSON) を読む
2. `CodeBlock` のうち class に `mermaid` を持つものを深さ優先で収集する
   （Div・BlockQuote・リスト内のブロックも対象）
3. 収集した Mermaid を `sekien::render_stream` で **一括** レンダリングする
4. per-block の結果を AST に適用する:
   - 成功 → `RawBlock("html", svg)` に置換
   - 失敗 → 元の `CodeBlock` をそのまま残し、stderr に warning を出す
5. 加工した AST を stdout に書き出す

文書内の N 個の Mermaid ブロックを `render_stream` 一回呼び出しでまとめて
処理するため、Xvfb / WebView / mermaid.js の初期化コストは 1 回分のみ。

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or https://www.apache.org/licenses/LICENSE-2.0)
- MIT License ([LICENSE-MIT](LICENSE-MIT) or https://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
