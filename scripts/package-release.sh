#!/usr/bin/env sh
set -eu

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
      exit 1
      ;;
  esac
}

target="${1:-$(detect_target)}"
dist_dir="target/dist"
staging_dir="$dist_dir/$target"
asset="kicad-ipc-cli-$target.tar.gz"
binary="target/release/kicad-ipc-cli"

cargo build --release --locked

mkdir -p "$staging_dir"
cp "$binary" "$staging_dir/kicad-ipc-cli"
chmod 755 "$staging_dir/kicad-ipc-cli"

tar -C "$staging_dir" -czf "$dist_dir/$asset" kicad-ipc-cli

if command -v sha256sum >/dev/null 2>&1; then
  (cd "$dist_dir" && sha256sum "$asset" > "$asset.sha256")
else
  (cd "$dist_dir" && shasum -a 256 "$asset" > "$asset.sha256")
fi

printf '%s\n' "$dist_dir/$asset"
