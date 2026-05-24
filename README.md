# kicad-ipc-cli

Demo-friendly command line companion for the KiCad PCB Editor IPC API.

This is a small Rust binary around [`kicad-ipc-rs`](https://crates.io/crates/kicad-ipc-rs) 0.5.0. It is shaped for screen-recordable KiCad demos and for agents that need structured JSON from a board without parsing `.kicad_pcb` files directly.

## Requirements

- Rust 1.82 or newer
- CMake, needed by the bundled `nng` transport build
- KiCad 10.0.1 or newer
- KiCad IPC API enabled

Enable IPC in KiCad:

1. Open KiCad.
2. Go to Preferences > Plugins.
3. Enable IPC API.
4. Restart KiCad, open a project, then open the PCB editor.

The socket is auto-detected by `kicad-ipc-rs`. Use `--socket <path>` and `--token <token>` when your setup needs explicit connection settings.

## Install

```bash
cargo install --path .
```

During development:

```bash
cargo run -- doctor
cargo run -- --format json inventory
```

## Output

Human summaries are the default for demos. Use JSON for scripts and agents:

```bash
kicad-ipc-cli --format json board-summary
```

Example shape:

```json
{
  "copper_layer_count": 2,
  "enabled_layers": [{ "id": 0, "name": "F.Cu" }],
  "net_count": 32
}
```

## Demo Recipes

```bash
# 1. Check connection, version, project, and open documents.
kicad-ipc-cli doctor

# 2. Summarize layers, origins, stackup, nets, and decoded item counts.
kicad-ipc-cli board-summary

# 3. Get a machine-readable inventory of footprints, pads, nets, and routing.
kicad-ipc-cli --format json inventory

# 4. Inspect the current selection with decoded item details.
kicad-ipc-cli selection --details

# 5. Report all net classes, then deep-dive a named net.
kicad-ipc-cli net-report
kicad-ipc-cli --format json net-report --net GND --connected

# 6. Generate a component grouping plan an AI or human can edit.
kicad-ipc-cli component-groups suggest --output groups.json

# 7. Apply a grouping plan to KiCad PCB groups.
kicad-ipc-cli --yes component-groups apply --plan groups.json

# 8. Select items visually by reference or net.
kicad-ipc-cli select by-ref U1 C1 J1
kicad-ipc-cli select by-net GND --mode replace

# 9. Adjust demo view state.
kicad-ipc-cli view active-layer 0
kicad-ipc-cli view preset focus-net --net GND

# 10. Inject a tutorial DRC marker.
kicad-ipc-cli --yes drc-marker --at 10mm,20mm --message "Inspect this clearance"

# 11. Refill all zones or selected zones.
kicad-ipc-cli --yes zones refill
kicad-ipc-cli --yes zones refill --selected

# 12. Save board or selection s-expression snapshots for review.
kicad-ipc-cli snapshot --scope board --output board-snapshot.kicad_pcb
kicad-ipc-cli snapshot --scope selection --output selected-items.kicad_pcb

# 13. Preview text extents and shape-conversion metadata.
kicad-ipc-cli text-shapes "CAN TERM"
```

## Component Groups

`component-groups suggest` builds a JSON plan from footprint references, values, pads, and net names. It recognizes demo-friendly groups such as connectors, MCU, CAN interface, power regulation, decoupling capacitors, and passives.

`component-groups apply` uses a KiCad commit session and real PCB group creation. It requires `--yes` and reports created group IDs when KiCad returns them. If KiCad rejects group creation, the command returns that API error instead of pretending success.

## Recording Ideas

- Start with `doctor`, then switch between the terminal and PCB editor.
- Use `inventory --format json` to show agent-readable board context.
- Generate `groups.json`, edit one group name, then apply it with `--yes`.
- Use `select by-net` and `view preset focus-net` to make KiCad visibly respond.
- Finish by writing a `snapshot` file and opening it in a text editor.

## CI Gates

The repository is expected to pass:

```bash
cargo fmt --all -- --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```
