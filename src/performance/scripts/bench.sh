#!/bin/bash
# Run the frame-time benchmark and append one tab-separated line per run to $RGA_BENCH_LOG.
#
#   src/performance/scripts/bench.sh <label> <runs> <experiment>...
#
# Any RGA_* switch in the environment passes through, e.g.
#   RGA_AA=smaa src/performance/scripts/bench.sh smaa 3 1 3
#
# Run from the package root after `cargo build --release`. RGA_BIN overrides the binary; the
# default is the release build in `target/`.
# Bevy 0.14 sometimes deadlocks on exit on macOS after the result is printed, so a run is given
# 45 seconds, killed if it is still alive, and its result kept with an `exit-hang` note.
set -u
label=$1; runs=$2; shift 2
bin=${RGA_BIN:-target/release/physics-sandbox}
log=${RGA_BENCH_LOG:-bench.tsv}
for experiment in "$@"; do
  for run in $(seq 1 "$runs"); do
    out=$(mktemp)
    RGA_BENCH=on "$bin" "$experiment" --seed 7 >"$out" 2>&1 &
    pid=$!
    for _ in $(seq 1 45); do kill -0 $pid 2>/dev/null || break; sleep 1; done
    note=""
    if kill -0 $pid 2>/dev/null; then kill -9 $pid 2>/dev/null; note=" exit-hang"; fi
    result=$(grep '^BENCH' "$out" | head -1 | sed 's/^BENCH //')
    [ -z "$result" ] && result="NO-RESULT $(grep -m1 -i 'error' "$out")"
    echo -e "$label\texp$experiment\trun$run\t$result$note" | tee -a "$log"
    rm -f "$out"
  done
done
