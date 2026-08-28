# Liminal Pipeline GUI Implementation Plan

## Product Direction

Build a GUI for designing, inspecting, validating, and eventually editing Liminal
pipeline TOML files. The TOML remains the source of truth: researchers should be able
to hand-edit configs and still trust the GUI to preserve comments, ordering, and
formatting wherever possible.

The visual direction should be similar in spirit to CyberEther's look and feel: a
technical, spatial, graph-oriented interface with a serious research-tool aesthetic.
The GUI should feel like an instrument for building pipelines, not a generic config
form or a marketing dashboard.

## Guiding Principles

- Resolve the graph model before building UI affordances.
- Keep Liminal's existing TOML schema authoritative.
- Use format-preserving edits with `toml_edit`; avoid parse-and-rewrite saves.
- Prefer generated palettes and parameter forms over hand-maintained UI per processor.
- Make each phase independently useful and testable.
- Surface validation errors visually as early as possible.

## Phase 1: Graph Foundation

Implement a standalone Rust graph resolver that converts `Config` into a GUI-neutral
intermediate graph model.

Suggested model:

```rust
ResolvedPipelineGraph {
    nodes: Vec<GraphNode>,
    edges: Vec<GraphEdge>,
    channels: Vec<GraphChannel>,
    diagnostics: Vec<GraphDiagnostic>,
}
```

Use stable, qualified node IDs:

```text
input:<name>
pipeline:<pipeline>.stage:<stage>
output:<name>
```

The resolver should:

- collect all input, pipeline-stage, and output nodes
- map each produced `output` channel to its producer node
- resolve each consumed `inputs = [...]` channel into explicit graph edges
- preserve channel metadata from the producing stage
- return diagnostics instead of hiding structural problems

Initial diagnostics:

- dangling input channel
- duplicate channel producer
- orphan produced channel
- cycle between transform stages
- name collision, if qualified IDs expose ambiguous display names

Initial tests:

- simple input -> stage -> output graph
- fan-out from one channel to multiple consumers
- dangling input channel
- duplicate output channel
- orphan produced channel
- cycle between pipeline stages
- cross-pipeline channel dependency
- existing `config/examples/*.toml` files

Done when the resolver can explain every channel edge in existing example configs
without involving Tauri or React.

## Phase 2: Intermediate GUI Contract

Define serializable DTOs for the Tauri boundary. These can mirror the Rust-native graph
model, but should be stable enough for the frontend to depend on.

Include:

- node ID
- node kind: input, pipeline stage, output
- display name
- processor type
- pipeline name, when applicable
- input channels
- output channel
- channel config
- edge channel name
- edge input index
- diagnostics and severity

Add a CLI or debug command that loads a TOML config, resolves it, and prints graph JSON.

Done when `config.toml -> graph.json` is deterministic, readable, and easy to diff.

## Phase 3: Read-Only GUI

Scaffold a Tauri + React app.

Backend responsibilities:

- open TOML file
- parse existing config structs
- run existing validation
- run the graph resolver
- return graph JSON to the frontend

Frontend responsibilities:

- render the graph with React Flow / xyflow
- arrange nodes into three lanes: Inputs, Pipeline Stages, Outputs
- show node names, processor types, channel labels, and diagnostics
- support file open and reload

Core interactions:

- fit to view
- minimap
- select node
- inspect node details
- inspect channel details
- highlight upstream/downstream paths
- search by node or channel name

Done when real existing configs can be opened and visually inspected.

## Phase 4: Validation UX

Add a diagnostics-first workflow so the read-only GUI is already useful for researchers
who currently debug TOML by hand.

UI requirements:

- diagnostics panel
- inline node and edge error states
- warning badges on lanes or sections
- direct navigation from diagnostic to affected graph item
- clear distinction between invalid config, unresolved graph, and non-fatal warnings

Done when common hand-authoring mistakes are easier to find in the GUI than in raw TOML.

## Phase 5: Format-Preserving TOML Editing

Introduce `toml_edit` for surgical mutation.

Keep both representations available:

- `toml_edit::DocumentMut` for edits and saves
- deserialized `Config` for validation, resolver input, and runtime compatibility

Start with existing-node edits:

- scalar processor parameters
- output channel string
- channel type
- channel capacity
- timing fields

Defer initially:

- stage rename
- processor type change
- input rewiring
- add/remove nodes

Round-trip tests should verify that only intended fields change and unrelated comments,
ordering, whitespace, and tables remain stable.

Done when editing one field and saving does not churn the TOML file.

## Phase 6: Processor Schema and Generated Forms

Add processor descriptors near the existing factory/registry layer.

Prefer a descriptor shape over only adding an associated function to `ProcessorConfig`,
because the GUI needs metadata even before constructing a processor.

Example:

```rust
ProcessorDescriptor {
    type_name: &'static str,
    category: ProcessorCategory,
    display_name: &'static str,
    params: Vec<FieldSpec>,
}
```

`FieldSpec` should support:

- string
- integer
- number
- boolean
- enum
- object
- array
- optional fields
- required fields
- defaults
- validation hints
- short help text

Nested objects and arrays are required because processors such as rule transforms use
tables and repeated `[[...parameters.rules]]` sections.

Done when the node palette and parameter forms are generated from processor descriptors
instead of hand-written React forms.

## Phase 7: Rewiring

Implement edge mutation as TOML edits.

Rules:

- connecting an edge adds the producer's output channel to the target stage's
  `inputs = [...]` array
- disconnecting an edge removes that channel from the target stage's inputs
- duplicate input entries are prevented or diagnosed
- channel settings are edited on the producer stage, since channel config belongs to
  the produced output

Done when visual rewiring updates TOML correctly and survives save/reload.

## Phase 8: Add and Remove Nodes

Build a generated palette from processor descriptors.

Palette groups:

- inputs
- transform stages
- aggregators
- outputs

Node creation should insert a new TOML table using defaults from the descriptor.

Node deletion should:

- warn when deleting a producer with downstream consumers
- surface the dangling channels that deletion would create
- support safe removal of now-unused channels and inputs

Done when a user can build a small valid pipeline from scratch.

## Phase 9: Layout, Polish, and Packaging

Add workflow polish:

- layout persistence
- dirty-state indicator
- manual save
- optional save-as
- recently opened configs
- keyboard shortcuts for common graph actions
- focused CyberEther-like visual styling

Layout metadata should probably live outside the TOML unless the project intentionally
adds a GUI metadata section.

Add integration tests that:

- load each example config
- resolve the graph
- edit a field
- save
- reload
- resolve again
- verify the expected graph and minimal diff

Package the app only after read, edit, save, and reload are trustworthy.

## Immediate Milestone

Start with Milestone 1: Resolver + JSON Export.

Deliverables:

- `src/config/graph.rs` or `src/core/graph.rs`
- `ResolvedPipelineGraph::from_config(&Config)`
- diagnostics for dangling, duplicate, orphan, and cycle cases
- unit tests against synthetic configs
- tests against existing example configs
- debug JSON export command or test

This milestone gives the whole GUI project a stable graph contract before any frontend
work begins.
