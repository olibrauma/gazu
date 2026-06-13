#!/bin/bash
# src/pandoc.rs の HTML_FORMATS allowlist を実測で再生成する。
#
# RawBlock("html", "<svg>...</svg>") を含む Pandoc AST を各 output format の
# writer に通し、SVG の中身がそのまま (またはフォーマット独自の raw-html 構文で)
# 出力されるフォーマットを "PASS" として一覧表示する。
#
# pandoc のバージョンを上げたときは、このスクリプトを再実行して
# src/pandoc.rs の HTML_FORMATS / コメントを見直すこと。
#
# 注意:
#   - chunkedhtml はマルチファイル writer (`-o <directory>` 必須) のため
#     stdout 経由のこの方法では検証できず、対象外にしている。
#   - docx/odt/pptx/epub 等のバイナリ formats も対象外（テキスト出力ではないため）。

set -euo pipefail

tmp=$(mktemp)
trap 'rm -f "$tmp"' EXIT

cat > "$tmp" <<'EOF'
{"pandoc-api-version":[1,23],"meta":{},"blocks":[{"t":"RawBlock","c":["html","<svg><circle r=\"5\"/></svg>"]}]}
EOF

skip="docx odt pptx epub epub2 epub3 rtf icml fb2 pdf bibtex biblatex csljson json native ipynb chunkedhtml"

for f in $(pandoc --list-output-formats); do
    case " $skip " in
        *" $f "*) continue ;;
    esac

    out=$(pandoc -f json -t "$f" "$tmp" 2>/dev/null || true)
    if echo "$out" | grep -q "circle"; then
        echo "PASS  $f"
    fi
done
