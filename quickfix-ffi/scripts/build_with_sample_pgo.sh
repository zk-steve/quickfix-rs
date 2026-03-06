#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  build_with_sample_pgo.sh --profile <sample-profile> -- <build command>

Example:
  build_with_sample_pgo.sh --profile /tmp/quickfix.prof -- cargo build -p gateway --release
EOF
}

profile_path=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --profile)
      profile_path="${2:-}"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    --)
      shift
      break
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage
      exit 1
      ;;
  esac
done

if [[ -z "$profile_path" ]]; then
  echo "Missing --profile <sample-profile>" >&2
  usage
  exit 1
fi

if [[ $# -eq 0 ]]; then
  echo "Missing build command after --" >&2
  usage
  exit 1
fi

export QUICKFIX_SAMPLE_PROFILE="$profile_path"
export QUICKFIX_LTO="${QUICKFIX_LTO:-thin}"

exec "$@"
