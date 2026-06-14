#!/bin/bash
# gazu benchmark — wall time + max RSS (all child processes included)
#
# Usage:
#   ./bench.sh
#   GAZU_BIN=../../target/release/gazu ./bench.sh
#   WARMUP_RUNS=5 BENCH_RUNS=20 ./bench.sh
#
# Output: Markdown table on stdout. Progress on stderr.
# If mermaid-filter (https://github.com/raghur/mermaid-filter) is on PATH,
# results are compared against it.
#
# Both filters are fed the same Pandoc AST (fixture.md, 3 Mermaid diagrams)
# on stdin with "html" as the output format, exactly as pandoc would invoke
# them via --filter.
#
# RSS is measured by walking the full PPID chain of the target process,
# capturing Xvfb/WebKit for gazu and node/Chromium for mermaid-filter.
# Sampled every 10 ms; the median peak value is reported.
#
# Dependencies: pandoc, ps, awk, sort, sed, date (GNU coreutils)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BINARY="${GAZU_BIN:-$SCRIPT_DIR/../../target/release/gazu}"
WARMUP_RUNS="${WARMUP_RUNS:-3}"
BENCH_RUNS="${BENCH_RUNS:-10}"
PUPPETEER_CFG="$SCRIPT_DIR/puppeteer-config.json"

[ -f "$BINARY" ] || {
    echo "Error: binary not found: $BINARY" >&2
    echo "hint: cargo build --release" >&2
    exit 1
}

AST_FILE=$(mktemp /tmp/gazu_bench_ast_XXXXXX.json)
DATAFILE=$(mktemp /tmp/gazu_bench_data_XXXXXX)
trap 'rm -f "$AST_FILE" "$DATAFILE"' EXIT

pandoc "$SCRIPT_DIR/fixture.md" -t json > "$AST_FILE"

has_mermaid_filter() { command -v mermaid-filter >/dev/null 2>&1; }

# Sum RSS of a process and all its descendants by walking the PPID chain (KB).
tree_rss_kb() {
    local root=$1
    ps -e -o pid= -o ppid= -o rss= | awk -v root="$root" '
    { ppid[$1]=$2; rss[$1]=$3; children[$2]=children[$2]" "$1 }
    END {
        q[0]=root; n=1; total=0
        while (n > 0) {
            p = q[--n]
            if (p == "") continue
            total += rss[p]
            split(children[p], c, " ")
            for (i in c) if (c[i] != "") q[n++] = c[i]
        }
        print total
    }'
}

# Run one measurement. Writes "elapsed_ms max_rss_kb" to $DATAFILE.
measure() {
    local t0 t1 max_rss=0 rss pid
    t0=$(date +%s%3N)
    "$@" >/dev/null 2>/dev/null &
    pid=$!
    while kill -0 "$pid" 2>/dev/null; do
        rss=$(tree_rss_kb "$pid")
        [ "${rss:-0}" -gt "$max_rss" ] && max_rss="${rss:-0}"
        sleep 0.01
    done
    wait "$pid" 2>/dev/null
    t1=$(date +%s%3N)
    printf '%d %d\n' "$(( t1 - t0 ))" "$max_rss" > "$DATAFILE"
}

# Lower median of a list of integers.
median() {
    local n=$#
    printf '%s\n' "$@" | sort -n | sed -n "$(( (n + 1) / 2 ))p"
}

# bench <cmd...> → echoes "median_ms median_rss_kb"
bench() {
    local i ms rss times=() rsses=()
    for (( i = 0; i < WARMUP_RUNS; i++ )); do
        "$@" >/dev/null 2>/dev/null
    done
    for (( i = 0; i < BENCH_RUNS; i++ )); do
        measure "$@"
        read -r ms rss < "$DATAFILE"
        times+=("$ms")
        rsses+=("$rss")
        printf '.' >&2
    done
    echo "$(median "${times[@]}") $(median "${rsses[@]}")"
}

fmt_ms() {
    local ms=$1
    if (( ms >= 1000 )); then
        awk "BEGIN { printf \"%.2f s\", $ms / 1000 }"
    else
        echo "${ms} ms"
    fi
}

fmt_rss() {
    awk "BEGIN { printf \"%.0f MB\", $1 / 1024 }"
}

run_gazu() {
    "$BINARY" html < "$AST_FILE"
}

run_mermaid_filter() {
    MERMAID_FILTER_FORMAT=svg \
    MERMAID_FILTER_PUPPETEER_CONFIG="$PUPPETEER_CFG" \
        mermaid-filter html < "$AST_FILE"
}

mf_present=false
has_mermaid_filter && mf_present=true
$mf_present || echo "note: mermaid-filter not in PATH — gazu only." >&2

echo "# gazu benchmark results"
echo "_(warmup ${WARMUP_RUNS} runs, measurement ${BENCH_RUNS} runs, median; fixture.md, 3 Mermaid diagrams)_"
echo ""

if $mf_present; then
    printf "| filter | time | RSS |\n"
    printf "|---|---|---|\n"
    printf "  %-20s" "gazu" >&2
    read -r g_ms g_rss < <(bench run_gazu)
    echo >&2
    printf "  %-20s" "mermaid-filter" >&2
    read -r m_ms m_rss < <(bench run_mermaid_filter)
    echo >&2
    printf "| gazu | %s | %s |\n" "$(fmt_ms "$g_ms")" "$(fmt_rss "$g_rss")"
    printf "| mermaid-filter | %s | %s |\n" "$(fmt_ms "$m_ms")" "$(fmt_rss "$m_rss")"
else
    printf "| filter | time | RSS |\n"
    printf "|---|---|---|\n"
    printf "  %-20s" "gazu" >&2
    read -r g_ms g_rss < <(bench run_gazu)
    echo >&2
    printf "| gazu | %s | %s |\n" "$(fmt_ms "$g_ms")" "$(fmt_rss "$g_rss")"
fi
