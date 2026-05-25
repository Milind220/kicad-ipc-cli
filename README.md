# kicad-ipc-cli

Give your coding agent eyes and hands inside KiCad.

`kicad-ipc-cli` is a small Rust binary that talks to a live KiCad PCB editor
through [`kicad-ipc-rs`](https://crates.io/crates/kicad-ipc-rs). It turns an
open board into deterministic JSON an agent can inspect, reason about, select,
snapshot, and edit without screen-scraping or brittle GUI automation.

Point an agent at a running board and it can answer questions like:

- What footprints, nets, pads, vias, zones, and tracks are on this PCB?
- Which pads and copper items are connected to `GND`?
- What is selected right now, and what exact KiCad item IDs does that map to?
- Can you select `U2`, snapshot it as KiCad S-expression, inspect its bbox, add
  temporary silkscreen text, then delete it and prove it is gone?

That last workflow is the point: agent PCB work should be observable,
undo-friendly, and boringly scriptable.

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/Milind220/kicad-ipc-cli/main/install.sh | sh
```

The installer downloads the latest prebuilt GitHub Release for Linux/macOS when
available, verifies the checksum when present, and installs into `~/.cargo/bin`.
If no matching asset exists, it falls back to `cargo install --git`.

Pin a release or install somewhere else:

```bash
KICAD_IPC_CLI_VERSION=v0.1.3 sh install.sh
KICAD_IPC_CLI_INSTALL_DIR=/usr/local/bin sh install.sh
KICAD_IPC_CLI_BUILD_FROM_SOURCE=1 sh install.sh
```

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

## Try It On A Live Board

These are the commands agents usually reach for first:

```bash
kicad-ipc-cli doctor
kicad-ipc-cli board-summary
kicad-ipc-cli inventory --limit 25
kicad-ipc-cli selection --details --limit 10
kicad-ipc-cli net-report --net GND --connected --limit 25
kicad-ipc-cli select by-id <uuid> --mode replace
kicad-ipc-cli select by-ref U1 C1 --mode replace
kicad-ipc-cli select by-net GND
kicad-ipc-cli snapshot --scope board --output board.kicad_pcb
```

Mutations that can change the board require `--yes`:

```bash
kicad-ipc-cli --yes api items create-board-text --text "REV A" --at 10mm,20mm --layer F.SilkS
kicad-ipc-cli --yes api items delete <uuid>
```

Delete output includes `verified_deleted`; follow-up missing checks can use:

```bash
kicad-ipc-cli api items get-by-id <uuid> --missing-ok
```

## Copy For Your Agents

Paste this into an agent prompt, repo note, or runbook:

```text
Use kicad-ipc-cli for live KiCad PCB inspection/editing. Install:
curl -fsSL https://raw.githubusercontent.com/Milind220/kicad-ipc-cli/main/install.sh | sh

Assume JSON output. Start with:
kicad-ipc-cli doctor
kicad-ipc-cli board-summary
kicad-ipc-cli inventory --limit 25
kicad-ipc-cli selection --details --limit 10

Useful workflows:
- Select parts: kicad-ipc-cli select by-ref U2 --mode replace
- Select raw IDs: kicad-ipc-cli select by-id <uuid> --mode replace
- Inspect a net: kicad-ipc-cli net-report --net GND --connected --limit 25
- Snapshot board/selection:
  kicad-ipc-cli snapshot --scope board --output /tmp/board.kicad_pcb
  kicad-ipc-cli snapshot --scope selection --output /tmp/selection.kicad_pcb
- Create temporary silkscreen text:
  kicad-ipc-cli --yes api items create-board-text --text "NOTE" --at 10mm,20mm --layer F.SilkS
- Delete and prove missing:
  kicad-ipc-cli --yes api items delete <uuid>
  kicad-ipc-cli api items get-by-id <uuid> --missing-ok

Rules:
- Do not save unless explicitly asked.
- Mutations need --yes.
- Prefer high-level commands first; use `kicad-ipc-cli api list` for raw escape hatches.
- Keep outputs small with --limit when available.
```

## Command Shape

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
proof, inspect `verified_deleted` in the delete output or follow up with
`api items get-by-id <uuid> --missing-ok`.

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

## Releases

Push a `v*` tag to publish release assets:

```bash
git tag v0.1.3
git push origin v0.1.3
```

The release workflow builds native archives named
`kicad-ipc-cli-<target>.tar.gz` plus `.sha256` files for Linux and macOS
runners. `install.sh` expects those asset names.
