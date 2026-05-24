#!/usr/bin/env sh
set -eu

repo="${KICAD_IPC_CLI_REPO:-https://github.com/Milind220/kicad-ipc-cli.git}"
ref="${KICAD_IPC_CLI_REF:-main}"

need() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "missing required command: $1" >&2
    exit 1
  }
}

need cargo
need git

cargo install --git "$repo" --branch "$ref" --locked kicad-ipc-cli
