# Codex task: build KiCad IPC CLI companion

You are implementing a new public Rust CLI repo: `kicad-ipc-cli`.

## Goal
Build a useful demo-quality CLI companion for KiCad PCB Editor using Milind's Rust bindings crate `kicad-ipc-rs`.

This should be shaped for screen-recordable demos and for AI agents to call from shell. It should expose ~10 genuinely useful commands over the KiCad v10 PCB/editor IPC surface.

## Context
- Binding crate: `kicad-ipc-rs = "0.5.0"`, use feature `blocking`.
- KiCad IPC API v10 is PCB/editor-focused. Assume KiCad 10.0.1+ with IPC enabled.
- We cannot run against live KiCad here. You must still compile, format, unit test what can be tested, and keep command contracts sane.
- Study `/root/kicad-ipc-rs/README.md` and `/root/kicad-ipc-rs/test-scripts/kicad-ipc-cli.rs` for available APIs and type names. Do NOT copy the giant test CLI wholesale; this repo should be polished and demo/product-shaped.

## CLI shape
Binary name: `kicad-ipc-cli`.
Use clap + serde/serde_json + anyhow.
Prefer JSON output by default where agents need it; allow human summaries for screen demos.
Global flags:
- `--socket <path>` optional override
- `--token <token>` optional
- `--timeout-ms <ms>` default 3000
- `--format human|json` default human for visual demos, json for agent commands when requested
- `--yes` required for mutating commands except harmless view changes if you choose to gate them too

## Commands: implement at least 10 useful demos
Design and implement a cohesive set. Suggested set:
1. `doctor` — ping KiCad, version, board-open, project path/open documents.
2. `board-summary` — layers, active layer, origins, stackup basics, net counts, item counts by type.
3. `inventory` — footprints with ref/value-ish fields when possible, pads, nets, tracks/vias/zones counts; agent-friendly JSON.
4. `selection` — summarize selected items deeply; optional `--json`/format output.
5. `net-report` — report net classes and for chosen nets show pads/items/connected items.
6. `component-groups suggest` — infer component clusters from references/net names/pads: power rails/regulators, CAN/transceivers, MCU, connectors, decouplers, passives. Output a JSON plan an AI can edit.
7. `component-groups apply --plan <file>` — create/refresh KiCad PCB groups from a JSON plan via commit session where API supports it. If API lacks ergonomic group creation, implement a safe placeholder that selects/moves/group-names only where possible and clearly errors with actionable message. Do not fake success.
8. `select` — add/remove/clear/select-by-ref/select-by-net to selection for visual demos.
9. `view` — active-layer, visible-layers, appearance presets (e.g. focus-net/ratsnest/all-layers) using IPC appearance APIs.
10. `drc-marker` — inject a DRC marker at coordinate or on selected items for tutorial annotations.
11. `zones refill` — refill all or selected zones.
12. `snapshot` — save board or selection to `.kicad_pcb`/sexpr file for AI context.
13. `text-shapes` — turn a label into shape/extents preview metadata.

Implement at least 10; more is okay if simple. Each mutating command must print the commit id/action or affected IDs.

## Code quality
- Keep modules small: `src/main.rs`, `src/cli.rs`, `src/client.rs`, `src/output.rs`, `src/commands/*` etc.
- No giant 3000-line single-file sludge. We are not raising a barn out of spaghetti.
- Add unit tests for pure logic: component group heuristics, output serialization, parsing units/refs/plans.
- Add README with: install, KiCad IPC enablement, 10 demo recipes, JSON examples, recording ideas.
- Add LICENSE MIT, .gitignore, and GitHub Actions CI for fmt/clippy/test/build.
- Atomic commits are encouraged if you can make them. If you cannot commit due to environment, leave clean diffs.

## Verification gates
Run:
- cargo fmt --all -- --check
- cargo test
- cargo clippy --all-targets -- -D warnings
- cargo build --release

If `cargo clippy` is unavailable, note it in final output.

## Commit policy
Make coherent atomic conventional commits if possible. If not, leave changes uncommitted and explain.
