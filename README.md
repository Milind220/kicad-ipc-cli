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
kicad-ipc-cli api board set-visible-layers F.Cu B.Cu F.SilkS
kicad-ipc-cli api board items --type track
kicad-ipc-cli api board items-by-net GND
kicad-ipc-cli api board bounding-boxes --item-id <uuid>
kicad-ipc-cli api selection get
kicad-ipc-cli api document title-block
kicad-ipc-cli --yes api document save
```

Layer arguments accept canonical names (`F.SilkS`), proto names
(`BL_F_SilkS`), or numeric IDs. PCB item filters accept friendly names such as
`track`, `trace`, `footprint`, `pad`, `text`, and `silkscreen-text`, with
`--type-code` retained as an alias for numeric filters.

Create/update/delete flows:

```bash
kicad-ipc-cli --yes api items create-board-text --text "REV A" --at 10mm,20mm
kicad-ipc-cli --yes api items create-board-text --text "DNP" --at 12mm,25mm --layer B.SilkS --height 0.8mm
kicad-ipc-cli --yes api items create-board-texts --file board-texts.json
kicad-ipc-cli --yes api items create-raw --file raw-items.json
kicad-ipc-cli --yes api items update-raw --file raw-items.json
kicad-ipc-cli --yes api items delete <uuid> <uuid>
```

Use `create-board-text`/`create-board-texts` for board text and silkscreen.
Those commands use typed `CreateItems`; `parse-create` is kept only for legacy
S-expression create flows and rejects board text snippets.

Raw command escape hatch:

```bash
kicad-ipc-cli api raw send --json '{"type_url":"type.googleapis.com/kiapi.common.commands.Ping"}'
```

Delete output means KiCad accepted the delete request. If a workflow needs
proof, follow up with `api items get-by-id <uuid>`.

Project/board updates:

```bash
kicad-ipc-cli --yes api common set-net-classes --file net-classes.json
kicad-ipc-cli --yes api board update-stackup --file stackup.json
```

Use `kicad-ipc-cli api list` for the binding coverage map and JSON schemas for
raw protobuf item payloads, board text payloads, net classes, and stackup
updates.

## CI Gates

```bash
cargo fmt --all -- --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```
