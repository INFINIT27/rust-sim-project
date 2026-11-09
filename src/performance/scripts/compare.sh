#!/bin/bash
# Interleaved A/B: run each configuration once per round, round after round, so drift (heat,
# background load) lands on every configuration equally instead of on whichever ran last.
#
#   src/performance/scripts/compare.sh <tag> <rounds> <experiment> <name:VAR=value,VAR=value>...
#
# e.g. src/performance/scripts/compare.sh aa 3 3 msaa4: smaa:RGA_AA=smaa fxaa:RGA_AA=fxaa
set -u
here=$(dirname "$0")
tag=$1; rounds=$2; experiment=$3; shift 3
for round in $(seq 1 "$rounds"); do
  for config in "$@"; do
    name=${config%%:*}; vars=${config#*:}
    (
      IFS=,
      for pair in $vars; do [ -n "$pair" ] && export "$pair"; done
      "$here/bench.sh" "$tag-$name" 1 "$experiment"
    ) 2>/dev/null | sed "s/\trun1\t/\tround$round\t/"
  done
done
