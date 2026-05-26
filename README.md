# kicad-ipc-cli

Let agents drive KiCad PCB Editor without writing a KiCad plugin.

`kicad-ipc-cli` is a small Rust CLI for KiCad's PCB Editor IPC API, built on [`kicad-ipc-rs`](https://crates.io/crates/kicad-ipc-rs). It gives an AI agent a direct command surface over a live KiCad board: inspect the PCB, get stable JSON, select items, create groups, add silkscreen, drop markers, refill zones, snapshot board state, and use raw IPC item operations when the high-level commands are not enough.

No screen scraping. No bespoke Python extension. No ritual sacrifice to the plugin goblin. Just commands your agent can call.

## What this is good for

Agents can use this to:

- inspect a live board: layers, stackup, nets, footprints, pads, tracks, vias, zones, groups, and current selection
- select by reference, net, or KiCad item ID so the PCB editor visibly responds
- group related components into KiCad PCB groups
- add or remove silkscreen labels and other board text
- place review markers for humans to inspect
- snapshot the whole board or current selection as KiCad S-expression
- refill zones, adjust view state, and focus KiCad on the relevant net/layer
- use raw IPC create/update/delete calls for advanced geometry work, like stitching vias, custom board-outline shapes, or exact item placement once you have the payload shape

The mental model: high-level commands for common agent workflows, raw IPC escape hatch for weird stuff. Weird stuff is where the fun lives.

## Install

Recommended:

```bash
curl -fsSL https://raw.githubusercontent.com/Milind220/kicad-ipc-cli/main/install.sh | sh
```

The installer downloads the latest prebuilt GitHub Release for Linux/macOS when available, verifies checksums when present, and installs into `~/.cargo/bin`. If no matching asset exists, it builds from source with Cargo.

Pin a release or change the install directory:

```bash
KICAD_IPC_CLI_VERSION=v0.1.4 sh install.sh
KICAD_IPC_CLI_INSTALL_DIR=/usr/local/bin sh install.sh
KICAD_IPC_CLI_BUILD_FROM_SOURCE=1 sh install.sh
```

From a checkout:

```bash
cargo install --path .
```

From GitHub source:

```bash
cargo install --git https://github.com/Milind220/kicad-ipc-cli.git --branch main --locked kicad-ipc-cli
```

## Requirements

- KiCad 10.0.1 or newer with PCB Editor IPC API v10
- a KiCad project open in PCB Editor for board-specific commands
- Rust 1.82+ and CMake if building from source

Enable IPC in KiCad:

1. Open KiCad.
2. Open **Preferences → Plugins**.
3. Enable the IPC API.
4. Restart KiCad.
5. Open your project and then open the PCB editor.

The socket is auto-detected in normal local setups. Use global `--socket <path-or-uri>` and `--token <token>` only when your environment requires explicit IPC connection settings.

## Agent quickstart

Copy/paste this into your agent context:

```text
You control a live KiCad PCB Editor through `kicad-ipc-cli`. Preconditions: KiCad is open, Preferences → Plugins → IPC API is enabled, and the PCB editor is open. JSON is default; use `--format human` only for people. Start every session: `kicad-ipc-cli doctor`; `kicad-ipc-cli board-summary`; `kicad-ipc-cli inventory --limit 50`; `kicad-ipc-cli selection --details --limit 20`. Before any board edit, create a visible temporary banner and save `created_text.id` as `<banner-id>`: `kicad-ipc-cli --yes api items create-board-text --text "AGENT CONTROLLING KICAD" --at 5mm,5mm --layer F.SilkS --height 2.5mm --stroke-width 0.35mm --bold`. Prefer high-level commands first: `select by-ref|by-net|by-id`, `component-groups apply` after authoring a group plan, `drc-marker`, `api board bounding-boxes`, `view preset focus-net`, `snapshot`. Use raw `api items create-raw|update-raw|delete|get-by-id` only when high-level commands are insufficient; inspect payloads before changing geometry. Verify every edit with the smallest useful readback (`board-summary`, `inventory`, `selection --details`, `api items get-by-id`, bounding boxes). Finish by deleting the banner: `kicad-ipc-cli --yes api items delete <banner-id>`; prove gone: `kicad-ipc-cli api items get-by-id <banner-id> --missing-ok`. Report exact refs/nets/item IDs touched. Never claim KiCad changed without CLI proof. The PCB editor is the truth; you are merely a caffeinated raccoon with shell access.
```

## Common workflows

### Inspect the board

```bash
kicad-ipc-cli doctor
kicad-ipc-cli board-summary
kicad-ipc-cli inventory --limit 50
kicad-ipc-cli net-report --net GND --connected --limit 50
kicad-ipc-cli api board bounding-boxes --item-id <uuid>
```

Bounding boxes exclude footprint reference/value and other child text by default for safer placement estimates. Add `--include-child-text` only when text extents are intentionally part of the measurement.

### Select things in the editor

```bash
kicad-ipc-cli select by-ref U1 C4 J2 --mode replace
kicad-ipc-cli select by-net GND --mode replace
kicad-ipc-cli select by-id <uuid> <uuid> --mode add
kicad-ipc-cli select clear
```

Selection mutation output is based on a readback after the mutation, not the mutation acknowledgement. Replace mode is preflighted where possible, then applied as clear followed by add because KiCad IPC does not provide atomic selection replace; if the add step fails after preflight, the previous selection may already be cleared.

### Change the view

```bash
kicad-ipc-cli view active-layer F.Cu
kicad-ipc-cli view preset focus-net --net GND
kicad-ipc-cli view preset ratsnest
kicad-ipc-cli api board set-visible-layers F.Cu B.Cu F.SilkS Edge.Cuts
```

View presets preserve unspecified appearance fields such as board flip. `view preset focus-net` also preflights and replaces the live selection with the focused net's items using the same non-atomic clear-then-add behavior as selection replace, and reports the actual selection readback.

### Group components

Agents decide the groups. The CLI only applies an explicit JSON plan — no baked-in MCU/CAN/USB horoscope.

Draft `groups.json` from `inventory`, `net-report`, board context, and actual design intent:

```json
{
  "version": 1,
  "generated_by": "agent-authored",
  "groups": [
    {
      "name": "Power input",
      "kind": "functional-block",
      "reason": "Connector, protection, and regulator belong together in this design",
      "references": ["J1", "F1", "U3"],
      "nets": ["VBUS", "3V3"]
    }
  ]
}
```

Then apply it:

```bash
kicad-ipc-cli --yes component-groups apply --plan groups.json
```

By default, `component-groups apply` deletes existing KiCad groups with matching names before creating replacements. Keep existing groups instead with:

```bash
kicad-ipc-cli --yes component-groups apply --plan groups.json --keep-existing
```

Useful agent moves:

- derive functional blocks from the actual schematic/board intent, then list explicit refs or item IDs
- use net reports as clues only; `generated_by`, `kind`, `reason`, and `nets` in `groups.json` are metadata and do not auto-expand membership
- keep group names unique; `apply` rejects duplicate names and explicit `item_ids` that do not resolve on the open board before starting the KiCad mutation
- create temporary groups before arranging or reviewing placement

### Add silkscreen or board text

Single label:

```bash
kicad-ipc-cli --yes api items create-board-text \
  --text "REV A" \
  --at 10mm,20mm \
  --layer F.SilkS \
  --height 1mm \
  --stroke-width 0.15mm
```

Batch labels:

```bash
kicad-ipc-cli --yes api items create-board-texts --file board-texts.json
```

`board-texts.json` can be either a bare array or `{ "board_texts": [...] }`:

```json
[
  { "text": "USB", "at": "12mm,8mm", "layer": "F.SilkS", "height": "1mm" },
  { "text": "CAN", "at": "30mm,8mm", "layer": "F.SilkS", "height": "1mm" }
]
```

### Add review markers

```bash
kicad-ipc-cli --yes drc-marker \
  --at 20mm,15mm \
  --severity warning \
  --message "Check connector clearance"
```

Attach a marker to selected items or explicit item IDs:

```bash
kicad-ipc-cli --yes drc-marker --selected --message "Review this cluster"
kicad-ipc-cli --yes drc-marker --item-id <uuid> --message "Verify this footprint"
```

### Snapshot exact board state

```bash
kicad-ipc-cli snapshot --scope board --output board.kicad_pcb
kicad-ipc-cli snapshot --scope selection --output selection.kicad_pcb
kicad-ipc-cli api document board-string
kicad-ipc-cli api document selection-string
```

This is useful before/after agent edits, for code review, or for handing exact KiCad S-expressions to another tool.

### Refill zones

```bash
kicad-ipc-cli --yes zones refill
kicad-ipc-cli --yes zones refill --selected
kicad-ipc-cli --yes zones refill --zone-id <zone-id>
```

`zones refill --selected` only refills selected zones. If no zones are selected and no explicit `--zone-id` values are supplied, the command fails instead of silently refilling everything. Sneaky footgun, defused.

Human distance inputs require explicit units such as `nm`, `um`, `mm`, `mil`, or `in`: use `--at 10mm,20mm`, not `--at 10,20`. Raw JSON fields ending in `_nm` remain raw integer nanometers.

### Raw IPC escape hatch

List exposed operations and schemas:

```bash
kicad-ipc-cli api list
```

Create, update, delete, and inspect raw items:

```bash
kicad-ipc-cli --yes api items create-raw --file raw-items.json
kicad-ipc-cli --yes api items update-raw --file raw-items.json
kicad-ipc-cli --yes api items delete <uuid> <uuid>
kicad-ipc-cli api items get-by-id <uuid> --missing-ok
kicad-ipc-cli api items get-editable-by-id <uuid>
```

Use this layer for advanced workflows that do not yet have friendly commands:

- adding stitching vias around a board edge or RF zone
- creating a particular shaped `Edge.Cuts` outline
- arranging footprints into a grid or logo-like pattern
- generating copper/text/graphic primitives from an external layout plan
- bulk-updating item coordinates after an agent computes placement

High-level commands should be your first move. Raw IPC is the crowbar. Useful, but do not wave it near your face.

## Agent rules

Give agents these rules when they use the CLI:

1. Start with `doctor`, `board-summary`, and a limited `inventory`.
2. Prefer JSON output and keep broad queries bounded with `--limit`.
3. Use high-level commands first: `select`, `component-groups`, `net-report`, `snapshot`, `drc-marker`, `zones`.
4. Use `api list` before raw IPC calls.
5. Any board-data mutation needs global `--yes`.
6. Add the `AGENT CONTROLLING KICAD` silk banner before edits; delete it before handoff.
7. Do not save the board unless the human explicitly asks.
8. After deletes, verify with `api items get-by-id <id> --missing-ok`; broad KiCad bad-request errors are ambiguous and are not treated as proof of deletion.
9. Snapshot before and after large edits.
10. If the editor feels stale, run `api common refresh`.

## Useful command map

```bash
# Health and context
kicad-ipc-cli doctor
kicad-ipc-cli board-summary
kicad-ipc-cli inventory --limit 50
kicad-ipc-cli selection --details --limit 20
kicad-ipc-cli net-report --net GND --connected --limit 50

# Selection and view
kicad-ipc-cli select by-ref U1 C1 --mode replace
kicad-ipc-cli select by-net GND --mode replace
kicad-ipc-cli view active-layer F.Cu
kicad-ipc-cli view preset focus-net --net GND

# Groups and annotations
# First author groups.json from inventory/net-report/board intent.
kicad-ipc-cli --yes component-groups apply --plan groups.json
kicad-ipc-cli --yes api items create-board-text --text "REV A" --at 10mm,20mm --layer F.SilkS
kicad-ipc-cli --yes drc-marker --at 10mm,20mm --message "Inspect this area"

# Board state and raw API
kicad-ipc-cli snapshot --scope board --output board.kicad_pcb
kicad-ipc-cli snapshot --scope selection --output selection.kicad_pcb
kicad-ipc-cli api list
kicad-ipc-cli api board items --type footprint --details
kicad-ipc-cli api board items-by-net GND --type pad
kicad-ipc-cli api board connected-items <uuid>
kicad-ipc-cli api board pad-shape-as-polygon --pad-id <pad-id> --layer F.Cu
kicad-ipc-cli api document title-block
```

Layer arguments accept KiCad names such as `F.SilkS`, proto names such as `BL_F_SilkS`, or numeric IDs. PCB item filters accept friendly names such as `track`, `trace`, `footprint`, `pad`, `text`, and `silkscreen-text`; numeric `--type-code` is still accepted.
Net arguments prefer exact net-name matches before numeric net codes, so a net literally named `12` resolves by name.

## Safety notes

Commands that edit board data require global `--yes`, including:

- `component-groups apply`
- `drc-marker`
- `zones refill`
- `api items create-board-text`
- `api items create-board-texts`
- `api items begin-commit`, `end-commit`
- `api items create-raw`
- `api items update-raw`
- `api items delete`
- `api raw send`
- `api common run-action`
- `api document save`, `save-copy`, `revert`, `set-title-block`

Selection and view commands change live editor state but do not edit board file data. Snapshot commands write output files and may overwrite the path you provide.

## Limitations

- Scope is KiCad PCB Editor IPC, not schematic editing.
- Binding target is KiCad IPC API v10 through `kicad-ipc-rs` 0.5.1.
- Commands need a running KiCad IPC server; tests do not launch KiCad.
- High-level commands cover common agent workflows. Advanced geometry uses raw IPC payloads until friendly commands exist.

## Development

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo check --all-targets
git diff --check
```

Build a release binary:

```bash
cargo build --release
./target/release/kicad-ipc-cli doctor
```

## Releases

Push a `v*` tag to publish release assets:

```bash
git tag v0.1.4
git push origin v0.1.4
```

The release workflow builds native archives named `kicad-ipc-cli-<target>.tar.gz` plus `.sha256` files for Linux and macOS runners. `install.sh` expects those asset names.
