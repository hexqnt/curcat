#!/usr/bin/env bash
set -euo pipefail

# Build curcat for RedOS inside a container (analogous to build_musl.sh).
# Result binary will be placed at ./target/redos/curcat on the host.

# You can override these via env vars before calling the script.
# Default base image as requested (Red Soft UBI 8):
REDOS_IMAGE="${REDOS_IMAGE:-registry.red-soft.ru/ubi8/ubi:latest}"
RUST_TOOLCHAIN="${RUST_TOOLCHAIN:-stable}"

# Basic CLI parsing to switch base image quickly.
usage() {
  cat <<USAGE
Usage: ./build_redos.sh [--ubi7|--ubi8] [--image IMAGE]

Options:
  --ubi7           Use registry.red-soft.ru/ubi7/ubi:latest as base
  --ubi8           Use registry.red-soft.ru/ubi8/ubi:latest as base (default)
  --image IMAGE    Use custom Docker image (overrides --ubi7/--ubi8)
  -h, --help       Show this help

Environment overrides:
  REDOS_IMAGE      Docker image to use (overridden by --image)
  RUST_TOOLCHAIN   Rust toolchain (default: stable)
USAGE
}

while [ $# -gt 0 ]; do
  case "$1" in
    --ubi7)
      REDOS_IMAGE="registry.red-soft.ru/ubi7/ubi:latest"
      shift
      ;;
    --ubi8)
      REDOS_IMAGE="registry.red-soft.ru/ubi8/ubi:latest"
      shift
      ;;
    --image)
      if [ $# -lt 2 ]; then echo "--image requires a value" >&2; exit 2; fi
      REDOS_IMAGE="$2"
      shift 2
      ;;
    --image=*)
      REDOS_IMAGE="${1#*=}"
      shift
      ;;
    -h|--help)
      usage; exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage; exit 2
      ;;
  esac
done

HOST_UID="$(id -u)"
HOST_GID="$(id -g)"
WORKDIR="$PWD"

echo "[build_redos] Using image: ${REDOS_IMAGE}"
docker pull "${REDOS_IMAGE}"

# We run as root in the container to install prerequisites, but keep host files owned by the host user.
docker run \
  --rm -t \
  -v "${WORKDIR}:/work" \
  -e HOST_UID="${HOST_UID}" \
  -e HOST_GID="${HOST_GID}" \
  -e RUST_TOOLCHAIN="${RUST_TOOLCHAIN}" \
  -e CARGO_HOME=/tmp/cargo \
  -e RUSTUP_HOME=/tmp/rustup \
  -e CARGO_TARGET_DIR=/tmp/target \
  -w /work \
  "${REDOS_IMAGE}" \
  /bin/sh -lc '
    set -euo pipefail

    # Ensure RUST_TOOLCHAIN has a default even with `set -u`
    : "${RUST_TOOLCHAIN:=stable}"

    # Detect package manager and install prerequisites
    install_pkgs() {
      if command -v dnf >/dev/null 2>&1; then
        dnf -y install curl ca-certificates gcc make pkgconfig tar xz gzip findutils which git shadow-utils || dnf -y install curl gcc make pkgconfig which
      elif command -v yum >/dev/null 2>&1; then
        yum -y install curl ca-certificates gcc make pkgconfig tar xz gzip findutils which git shadow-utils || yum -y install curl gcc make pkgconfig which
      elif command -v microdnf >/dev/null 2>&1; then
        microdnf install -y curl ca-certificates gcc make pkgconfig tar xz gzip findutils which git shadow-utils || microdnf install -y curl gcc make pkgconfig which
      elif command -v apt-get >/dev/null 2>&1; then
        apt-get update && apt-get install -y curl ca-certificates build-essential pkg-config git
      else
        echo "No supported package manager found (dnf/yum/microdnf/apt-get)." >&2
        exit 1
      fi
    }

    echo "[build_redos] Installing prerequisites..."
    install_pkgs

    echo "[build_redos] Installing Rust toolchain (${RUST_TOOLCHAIN})..."
    curl -fsSL https://sh.rustup.rs | sh -s -- -y --default-toolchain "${RUST_TOOLCHAIN}"
    export PATH="/tmp/cargo/bin:$PATH"

    echo "[build_redos] Building (cargo build --release)..."
    /tmp/cargo/bin/cargo build --release

    echo "[build_redos] Preparing output..."
    mkdir -p /work/target/redos
    if command -v strip >/dev/null 2>&1; then
      strip /tmp/target/release/curcat || true
    fi
    cp -a /tmp/target/release/curcat /work/target/redos/curcat

    # Ensure host user owns the result
    chown -R "$HOST_UID:$HOST_GID" /work/target/redos || true

    echo "[build_redos] Done. Binary at ./target/redos/curcat"
  '
