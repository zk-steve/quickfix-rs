#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  apply_bolt.sh --binary <path> --perf-data <path> [--output <path>]

Environment overrides:
  PERF2BOLT_BIN  (default: perf2bolt)
  LLVM_BOLT_BIN  (default: llvm-bolt)

Example:
  apply_bolt.sh --binary target/release/gateway --perf-data /tmp/perf.data
EOF
}

binary_path=""
perf_data_path=""
output_path=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --binary)
      binary_path="${2:-}"
      shift 2
      ;;
    --perf-data)
      perf_data_path="${2:-}"
      shift 2
      ;;
    --output)
      output_path="${2:-}"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage
      exit 1
      ;;
  esac
done

if [[ -z "$binary_path" || -z "$perf_data_path" ]]; then
  echo "Both --binary and --perf-data are required." >&2
  usage
  exit 1
fi

if [[ -z "$output_path" ]]; then
  output_path="${binary_path}.bolt"
fi

perf2bolt_bin="${PERF2BOLT_BIN:-perf2bolt}"
llvm_bolt_bin="${LLVM_BOLT_BIN:-llvm-bolt}"
fdata_path="${output_path}.fdata"

"$perf2bolt_bin" "$binary_path" -p "$perf_data_path" -o "$fdata_path"
"$llvm_bolt_bin" "$binary_path" \
  -o "$output_path" \
  -data "$fdata_path" \
  -reorder-blocks=ext-tsp \
  -reorder-functions=hfsort+ \
  -split-functions=3 \
  -split-all-cold \
  -icf=1 \
  -dyno-stats

echo "BOLT optimized binary written to: $output_path"
