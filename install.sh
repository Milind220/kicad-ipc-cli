#!/usr/bin/env sh
set -eu

github_repo="${KICAD_IPC_CLI_GITHUB_REPO:-Milind220/kicad-ipc-cli}"
git_repo="${KICAD_IPC_CLI_GIT_REPO:-${KICAD_IPC_CLI_REPO:-https://github.com/Milind220/kicad-ipc-cli.git}}"
version="${KICAD_IPC_CLI_VERSION:-latest}"
install_dir="${KICAD_IPC_CLI_INSTALL_DIR:-$HOME/.cargo/bin}"
build_from_source="${KICAD_IPC_CLI_BUILD_FROM_SOURCE:-0}"

need() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "missing required command: $1" >&2
    exit 1
  }
}

detect_target() {
  os="$(uname -s)"
  arch="$(uname -m)"
  case "$os:$arch" in
    Darwin:arm64) echo "aarch64-apple-darwin" ;;
    Darwin:x86_64) echo "x86_64-apple-darwin" ;;
    Linux:x86_64) echo "x86_64-unknown-linux-gnu" ;;
    Linux:aarch64 | Linux:arm64) echo "aarch64-unknown-linux-gnu" ;;
    *)
      echo "unsupported platform: $os $arch" >&2
      return 1
      ;;
  esac
}

install_from_source() {
  need cargo
  need git

  if [ -n "${KICAD_IPC_CLI_REF:-}" ]; then
    cargo install --git "$git_repo" --branch "$KICAD_IPC_CLI_REF" --locked kicad-ipc-cli
  elif [ "$version" = "latest" ]; then
    cargo install --git "$git_repo" --branch main --locked kicad-ipc-cli
  else
    cargo install --git "$git_repo" --tag "$version" --locked kicad-ipc-cli
  fi
}

verify_sha256() {
  archive="$1"
  checksum_file="$2"
  checksum_dir="$(dirname "$checksum_file")"
  checksum_name="$(basename "$checksum_file")"

  if command -v sha256sum >/dev/null 2>&1; then
    (cd "$checksum_dir" && sha256sum -c "$checksum_name")
  elif command -v shasum >/dev/null 2>&1; then
    (cd "$checksum_dir" && shasum -a 256 -c "$checksum_name")
  else
    echo "warning: no sha256 checker found; skipping checksum for $archive" >&2
  fi
}

install_prebuilt() {
  if ! command -v curl >/dev/null 2>&1 || ! command -v tar >/dev/null 2>&1; then
    echo "curl or tar unavailable; building from source" >&2
    install_from_source
    return
  fi

  if ! target="$(detect_target)"; then
    echo "prebuilt binary unavailable for this platform; building from source" >&2
    install_from_source
    return
  fi
  asset="kicad-ipc-cli-$target.tar.gz"
  tmpdir="$(mktemp -d)"
  archive="$tmpdir/$asset"
  checksum="$tmpdir/$asset.sha256"

  if [ "$version" = "latest" ]; then
    base_url="https://github.com/$github_repo/releases/latest/download"
  else
    base_url="https://github.com/$github_repo/releases/download/$version"
  fi

  if ! curl -fsSL "$base_url/$asset" -o "$archive"; then
    echo "prebuilt binary unavailable for $target; building from source" >&2
    install_from_source
    return
  fi

  if curl -fsSL "$base_url/$asset.sha256" -o "$checksum"; then
    verify_sha256 "$archive" "$checksum"
  else
    echo "warning: checksum unavailable for $asset" >&2
  fi

  tar -xzf "$archive" -C "$tmpdir"
  mkdir -p "$install_dir"
  cp "$tmpdir/kicad-ipc-cli" "$install_dir/kicad-ipc-cli"
  chmod 755 "$install_dir/kicad-ipc-cli"
  echo "installed kicad-ipc-cli to $install_dir/kicad-ipc-cli"
}

if [ "$build_from_source" = "1" ]; then
  install_from_source
else
  install_prebuilt
fi
