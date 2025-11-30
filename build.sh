#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT_DIR"

if ! command -v zip >/dev/null 2>&1; then
  echo "[build] 'zip' utility is required. Install it and rerun the script." >&2
  exit 1
fi

package_name="$(sed -n 's/^name[[:space:]]*=[[:space:]]*\"\\(.*\\)\"/\\1/p' Cargo.toml | head -n 1)"
package_version="$(sed -n 's/^version[[:space:]]*=[[:space:]]*\"\\(.*\\)\"/\\1/p' Cargo.toml | head -n 1)"
PACKAGE_NAME="${package_name:-curcat}"
PACKAGE_VERSION="${package_version}"

./build_musl.sh
./build_redos.sh --ubi7

package_binary() {
  local binary_path="$1"
  local suffix="$2"
  local archive="${ROOT_DIR}/${PACKAGE_NAME}-${PACKAGE_VERSION}-${suffix}.zip"

  if [ ! -f "$binary_path" ]; then
    echo "[build] Expected binary not found: $binary_path" >&2
    exit 1
  fi

  echo "[build] Packaging ${binary_path} -> $(basename "$archive")"
  rm -f "$archive"

  local files=("$binary_path")
  if [ -f "${ROOT_DIR}/Readme.md" ]; then
    files+=("${ROOT_DIR}/Readme.md")
  fi

  zip -j "$archive" "${files[@]}"
}

package_binary "${ROOT_DIR}/target/x86_64-unknown-linux-musl/release/curcat" "linux-musl"
package_binary "${ROOT_DIR}/target/redos/curcat" "redos-ubi7"

echo "[build] Archives saved to ${ROOT_DIR}"
