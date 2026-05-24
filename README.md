# kicad-ipc-cli

Rust CLI for AI agents controlling a running KiCad PCB editor through
[`kicad-ipc-rs`](https://crates.io/crates/kicad-ipc-rs).

The CLI favors deterministic JSON, explicit IDs, and non-interactive commands.
It is a binary target intended for `cargo install`, release artifacts, or a
small installer script that coding agents can invoke from terminals.

## Requirements

- Rust 1.82 or newer
- CMake, needed by the bundled `nng` transport build
- KiCad 10.0.1 or newer
- KiCad IPC API enabled

Enable IPC in KiCad:

1. Open KiCad.
2. Preferences > Plugins > Enable IPC API.
3. Restart KiCad.
4. Open a project and the PCB editor.

The socket is auto-detected by `kicad-ipc-rs`. Use `--socket <path>` and
`--token <token>` when the environment needs explicit connection settings.

## Install

```bash
cargo install --path .
```

Remote install:

```bash
curl -fsSL https://raw.githubusercontent.com/Milind220/kicad-ipc-cli/main/install.sh | sh
```

Development:

```bash
cargo run -- api list
cargo run -- api common version
cargo run -- api board nets
```

## Output

JSON is the default.

```bash
kicad-ipc-cli api board active-layer
kicad-ipc-cli --format human doctor
```

All board mutations require `--yes` when the command can change the document.
Selection and view-state commands are intentionally lightweight because agents
often use them for visual targeting.

## Command Shape

High-level agent commands:

```bash
kicad-ipc-cli doctor
kicad-ipc-cli board-summary
kicad-ipc-cli inventory
kicad-ipc-cli selection --details
kicad-ipc-cli net-report --net GND --connected
kicad-ipc-cli select by-ref U1 C1 --mode replace
kicad-ipc-cli select by-net GND
kicad-ipc-cli snapshot --scope board --output board.kicad_pcb
```

Direct binding groups:

```bash
kicad-ipc-cli api common version
kicad-ipc-cli api common open-documents pcb
kicad-ipc-cli api common run-action pcbnew.InteractiveRouter
kicad-ipc-cli api board enabled-layers
kicad-ipc-cli api board set-visible-layers 0 31
kicad-ipc-cli api board items --type-code 1
kicad-ipc-cli api board items-by-net GND
kicad-ipc-cli api board bounding-boxes --item-id <uuid>
kicad-ipc-cli api selection get
kicad-ipc-cli api document title-block
kicad-ipc-cli --yes api document save
```

Create/update/delete flows:

```bash
kicad-ipc-cli --yes api items parse-create --file items.kicad_pcb
kicad-ipc-cli --yes api items create-raw --file raw-items.json
kicad-ipc-cli --yes api items update-raw --file raw-items.json
kicad-ipc-cli --yes api items delete <uuid> <uuid>
```

Project/board updates:

```bash
kicad-ipc-cli --yes api common set-net-classes --file net-classes.json
kicad-ipc-cli --yes api board update-stackup --file stackup.json
```

Use `kicad-ipc-cli api list` for the binding coverage map and JSON schemas for
raw protobuf item payloads, net classes, and stackup updates.

## CI Gates

```bash
cargo fmt --all -- --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```
