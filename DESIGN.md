# Liminal Pipeline GUI — Design Notes

## Motivation

Researchers using Liminal like the tool but consistently grumble about hand-authoring
TOML pipeline configs. The goal is a GUI that lets them visually build, inspect, and edit
Liminal pipelines, without giving up the TOML files as the source of truth (some
researchers will still hand-edit them alongside the GUI).

This is being scoped **independently** of a separate, longer-term idea (composable
GPU-processing stages in Liminal, inspired by CyberEther's tensor/handle model). The GUI
work does not depend on that and should be built first.

## Core problem: edges are implicit, not explicit

Liminal's TOML schema has no `from_node` / `to_node` concept. Nodes reference **channel
names**, not other nodes:

```toml
[inputs.sensor_data]
type = "simulated"
output = "raw_channel"          # this node PRODUCES "raw_channel"

[pipelines.p1.stages.filter]
type = "rule"
inputs = ["raw_channel"]        # this node CONSUMES "raw_channel"
output = "filtered_channel"

[outputs.log]
type = "console"
inputs = ["filtered_channel"]
```

An edge only exists because two nodes happen to reference the same channel name. Before
any graph can be drawn, this has to be resolved explicitly:

```rust
// Pass 1: collect every declared output channel -> producing node
let mut channel_producers: HashMap<String, NodeId> = HashMap::new();

// Pass 2: for every consumer's `inputs = [...]`, look up the producer
// and materialize an explicit edge (producer_channel -> consumer)
```

This resolver is the **load-bearing piece** of the whole project. Getting it right early
matters more than anything visual, because:

- Fan-out (multiple stages listing the same channel in `inputs`) becomes multiple edges
  from one producer — maps naturally to `broadcast` / `fanout` channel semantics.
- An `inputs` entry with no matching producer is a validation error ("stage X references
  channel 'foo' but nothing produces it") — likely the single most common
  hand-authoring mistake today, and surfacing it visually is high-value on its own,
  even before any editing capability exists.

**Action item:** build the resolver as a standalone Rust module with unit tests
(dangling-reference detection, fan-out grouping) before touching any UI code.

## Round-tripping TOML: use `toml_edit`, not `toml`

Some researchers will keep hand-editing configs alongside the GUI. A naive
parse-to-struct-and-reserialize approach (plain `toml` + `serde`) will silently destroy
comments and reorder tables on save — which would erode trust in the tool immediately.

Use **`toml_edit`** instead: a format-preserving document model that allows surgical
mutation (rename a key, insert a table, reorder an array) while leaving everything else
byte-identical to the original file.

## Stack

- **Tauri** app: Rust backend reuses Liminal's own config structs and `toml_edit`
  directly — no schema duplication across languages/processes.
- **react-flow (xyflow)** for the canvas. Node-graph interaction (drag to connect,
  minimap, auto-layout) is a solved problem in this library; hand-rolling a graph canvas
  in something like `egui` is a multi-week detour with no payoff for this project.
- Three loosely-columned lanes — **Inputs / Pipeline stages / Outputs** — mirroring the
  TOML's own top-level sections (`[inputs]`, `[pipelines]`, `[outputs]`). This gives a
  sensible default layout for free, and matches how researchers already mentally model
  the file, rather than a generic force-directed graph layout.

## Palette and parameter forms: generate, don't hand-write

Processor parameters currently only exist inside each processor's own
`ProcessorConfig::from_stage_config` implementation — there's no reflection available to
build a form from. Hand-writing a UI form per processor type will rot the moment someone
adds a new processor.

**Proposed trait addition** (small, low-risk change to Liminal itself):

```rust
trait ProcessorConfig {
    fn from_stage_config(config: &StageConfig) -> anyhow::Result<Self>;
    fn validate(&self) -> anyhow::Result<()>;
    fn param_schema() -> Vec<FieldSpec>;  // NEW: name, type, default, required
}
```

With this in place, the node palette and every parameter form can be **generated** from
the registry rather than maintained by hand. New processors appear in the GUI
automatically the moment they're registered with the factory.

## Build order (phased, each phase independently verifiable)

1. **Phase 1a — Resolver.** Standalone Rust module, no UI. Unit tests: dangling
   references, fan-out grouping, cycle detection if relevant. This is the foundation
   everything else depends on; get it solid first.

2. **Phase 1b — Read-only viewer.** Tauri scaffold + react-flow rendering the resolver's
   output, read-only, against real example configs (`config/examples/*.toml`). No
   editing, no serialization yet. Forces validation of the resolver and layout against
   real files before mutation is introduced. Independently useful on its own — being
   able to *see* pipeline shape is a big chunk of the "TOML is annoying" complaint,
   even without editing.

3. **Phase 2 — Parameter editing + `toml_edit` round-trip.** Edit values on existing
   nodes, save back to disk. Verify with a diff test: only the intended fields change,
   nothing else in the file moves or gets reformatted.

4. **Phase 3 — Rewiring.** Drag a connection to mutate a target stage's `inputs` array
   (and pick/confirm a channel type); delete a connection likewise.

5. **Phase 4 — Add/remove nodes** from the generated palette. Depends on
   `param_schema()` existing (from the trait addition above) and a solid default-value
   story, so it comes last.

## Explicitly out of scope for this effort

- Live-editing a *running* pipeline (would require something like CyberEther's
  mutation-lock / incomplete-block-retry machinery). Assume edit-then-restart is
  acceptable for now.
- Multi-backend GPU interop, composable GPU stages, opaque device-resident handles —
  a separate, later effort. Not a dependency of the GUI work.
- Schema/type validation of GPU tensor shapes at graph-build time — not relevant until
  GPU stages exist.

## Open question to settle early in implementation

What's the intermediate graph JSON model — the representation that sits between "parsed
TOML + resolved edges" (Rust side) and "what react-flow actually renders" (frontend
side)? This shape will influence both the resolver's output format and the frontend
component design, so it's worth nailing down before writing the Tauri IPC boundary.
