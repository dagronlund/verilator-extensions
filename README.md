# verilator-extensions

`verilator-extensions` is a Cargo workspace for parsing Verilator JSON ASTs,
converting them to AIGER, and generating standalone Rust simulators.

Run this example from the repository root with a Verilator build supporting
`-fno-delayed`, `--ast-pre-codegen`, and `--sva-preserve`:

```sh
# Generate the JSON AST from the counter fixture.
mkdir -p build
verilator --cc -fno-table -fno-delayed --coverage-user --sva-preserve \
  --top-module counter --Mdir build/counter-verilator \
  --ast-pre-codegen build/counter.json tests/counter/tb.sv

# Convert the same JSON AST to AIGER.
cargo run -p verilator-formal -- build/counter.json \
  --clock clk --reset '!reset_n' --output build/counter.aig

# Generate a Rust simulator library from the same JSON AST.
cargo run -p verilator-rust -- build/counter.json \
  --clock clk --output build/generated-counter --crate-name generated-counter
```

The generated Rust project directory must be new or empty.

The workspace contains the following crates:

- `verilator-parser` parses and validates JSON into an owned, strongly typed
  Rust AST. It is independent of the FSM representation.
- `verilator-utils` provides the Boolean FSM, signed variables, three-valued
  simulation, and ASCII/binary AIGER I/O without third-party dependencies.
- `verilator-formal` depends on `verilator-parser` and converts its AST into an
  ordered Boolean finite-state machine and AIGER. The FSM is represented by the
  sibling `verilator-utils` crate, which provides the Boolean gate, register,
  formal-property, ordering, and AIGER-writing APIs.
- `verilator-rust` lowers the typed AST directly into a Rust library that uses
  the sibling `verilator-rust-runtime` package and simulates one selected RTL
  clock edge per call.

The library converts Verilator JSON in two explicit phases. It first parses and
validates the supported synchronous subset into an owned, strongly typed Rust
AST, then orders and bit-blasts that AST into a named `verilator-utils` FSM ready for
ASCII or binary AIGER 1.9 serialization. The CLI performs the same conversion
and prints a concise summary. The supported regression inputs are tracked in
`tests/`.

Target Workflow
---------------

Run `./test.sh` to remove each fixture's `build/` directory, rebuild every
Verilator fixture, and run all Cargo workspace tests. Additional arguments are
passed to `cargo test`.

Each regression fixture contains its SystemVerilog sources and a Ninja build.
Use a Verilator build that supports `--sva-preserve`, `--ast-pre-codegen`, and
`-fno-delayed`.
To rebuild a single fixture, run Ninja in its directory, for example:

```sh
ninja -C tests/counter
```

Each Ninja build keeps all Verilator output in its local `build/` directory and
uses `-fno-delayed --ast-pre-codegen build/ast.json` to write the JSON AST before
scheduling and C++ generation. The Rust tests invoke Ninja once per fixture and
validate the stable AST. Missing or invalid cached ASTs are cleaned and rebuilt
once automatically. Gecko's simulator targets separately generate C++ before
compiling the simulator; those C++ generation commands must omit `-fno-delayed`.
ASTs containing lowered NBA phases are rejected with instructions to regenerate
them. The AST does not record command-line provenance, so a tree without NBA
operations cannot always be distinguished from one generated without the flag.

Convert the tree, print a concise named FSM summary, and optionally write
AIGER 1.9. The output extension selects ASCII `.aag` or binary `.aig`:

```sh
cargo run -- \
  tests/counter/build/ast.json \
  --clock clk \
  --reset '!reset_n' \
  --output counter.aag

cargo run -- \
  tests/counter/build/ast.json \
  --clock clk \
  --output counter.aig

cargo run -- \
  tests/counter/build/ast.json \
  --clock clk \
  --ron-output counter.ron
```

`--clock` is required and `--reset` is optional. Prefix a clock with `!` to
select its negative edge, or a reset with `!` to select active-low polarity,
for example `--clock '!clkn' --reset '!reset_n'`.
The converter rejects sequential sensitivity trees whose clock name or edge
does not match. For a specified reset, the converter first builds the FSM with
ordinary unknown initialization, simulates one transition with reset asserted,
and uses the resulting known latch values as initial values. It then replaces
the reset input with its inactive constant, so the reset is not present in the
exported AIGER inputs. A reset that is not specified remains an ordinary input.

Pass `--ron-output PATH` to export the parsed, strongly typed `ast::Design` as
pretty-printed RON. This export occurs before bit blasting and can be requested
with or without an AIGER `--output`.

Pass `--strip-symbols` to omit all input, latch, output, and property names from
the exported AIGER file. Pass `--debug` to print the exact symbol records that
would normally be written; this also works together with `--strip-symbols`.
Pass `--zero-init` to force every register bit to have an initial value of zero
in the exported AIGER file, including registers whose initial value would
otherwise be unknown or one.

Pass `--assertion INDEX` (or `--assert INDEX`) to export only that assertion, or
`--cover INDEX` to export only that cover, inverted and encoded as an assertion.
Indices are zero-based within their respective property lists and follow the
converter's property order. The two options are mutually exclusive and require
`--output`; assumptions remain in the exported model.

Model
-----

The converter produces a named wrapper around `verilator_utils::fsm::FSM` containing
the selected clock domain and ordered names for inputs, registers, outputs,
assertions, assumptions, and covers. These names are also stored as native
FSM labels, so its AIGER readers and writers preserve them directly.
Packed vectors are represented
least-significant bit first while retaining their original SystemVerilog bit
indices.

The typed AST preserves fixed-width Verilator dtype structure for aliases,
enums, packed arrays, packed structs, packed unions, and nested unpacked arrays.
Each dtype also carries a canonical flattened width, signedness, and layout for
conversion. Runtime containers, object/interface types, unpacked structs, and
tagged unions are rejected when referenced.

The design scope hierarchy is selected through `TOPSCOPE`/`SCOPE`. Executable
blocks are collected from every descendant scope, and variables are identified
by their instance-specific `varScopep` references. Other AST objects are
resolved through Verilator's address references (`addr`, `varp`, and `dtypep`)
rather than display names.

The parser retains every posedge/negedge sensitivity domain without selecting a
clock or reset. During conversion, the requested clock must match one of those
domains; other encountered edge domains are rejected unless they match the
requested reset. One FSM transition represents one selected clock edge, so the
clock is not an FSM input. Primary inputs and readable signals without drivers
become nondeterministic inputs. Registers without a recognized constant
initializer have an unknown initial value. A specified reset is applied once by
three-valued simulation to infer resettable latch values, then tied inactive in
the final FSM. Verilator-generated `_Vpast_*` history and `__Vnfa_*` assertion-state registers
are initialized to zero, matching Verilator's initialization convention. Their
explicit zero-valued `INITIALSTATIC` assignments are accepted by AIGER conversion;
other initial blocks remain unsupported.

The parser preserves `ASSIGNDLY` as `AssignmentKind::Nonblocking`. Both backends
implement its scheduling directly:

- Sampled expressions are captured before executing clocked processes.
- Blocking assignments update the execution environment immediately.
- Nonblocking assignments evaluate their RHS and destination indices when the
  statement executes, and accumulate updates separately from readable values.
- All pending updates commit after all clocked processes finish. Unwritten bits
  retain their values; later overlapping writes within one process take priority.
- Conditional updates become muxes in the formal model and branches in Rust.

State is identified from clocked writes and blocking values that must persist
between edges; blocking temporaries assigned before use need no registers.
The typed AST no longer contains NBA shadow-register pairs or pre/post lowering
phases. Previously serialized RON designs must be regenerated.

Mixed blocking/nonblocking writes to the same variable, overlapping writes from
multiple clocked processes, and cross-process reads of blocking-written variables
are rejected. Separate processes may write statically disjoint bit lanes, including
lanes of dynamically indexed memories. Nonblocking assignments outside clocked
processes are unsupported.

Formal properties use the durable `__Vsva_*` variables. Assert wires are
violation indicators and are inverted before being added as FSM assertions;
assume and cover wires are used directly. Covers remain in the in-memory model
but are omitted from AAG because the AIGER writer has no cover section.

Supported RTL Subset
--------------------

The converter supports:

- constants and variable references;
- bitwise, logical, and reduction AND/OR/XOR/NOT;
- addition, subtraction, multiplication, signed/unsigned division, and unary
  negation;
- equality and signed/unsigned relational comparisons;
- conditional muxes;
- logical left, logical right, and arithmetic right shifts;
- concatenation, packed replication, constant-width bit/part selection with
  constant or dynamic offsets, and zero/sign extension;
- multidimensional unpacked memories with packed scalar elements and dynamic
  element reads/writes;
- whole-variable and constant-width select assignments; and
- blocking/nonblocking assignments with nested `IF` and `case` statements.

Generate ASTs containing `case` statements with Verilator's
`-fno-table` option. Otherwise, Verilator may replace them with unpacked
constant lookup tables, which remain outside the supported subset.

Word-level operations use the `verilator-formal` crate's `ops::FsmOps`
extension trait, including
XOR, addition, subtraction, multiplication, signed/unsigned division,
comparison, mux, select, shift, and rotate construction. Selections use
LSB-first vector slices, with dynamic offsets lowered to muxes. This crate
remains responsible for Verilator AST dispatch, operand sizing, signedness
validation, procedural semantics, and symbol metadata.

Library users can keep the phases separate:

```rust
let document = verilator_parser::document::AstDocument::from_reader(reader)?;
let clock = verilator_parser::ast::Domain::new(
    "clk",
    verilator_parser::ast::Edge::Positive,
);
let design = verilator_parser::ast::Design::try_from(&document)?;
let model = verilator_formal::convert::NamedFsm::from_design(&design, clock, None)?;
```

For a single-step conversion, pass the same signal specifications to
`NamedFsm::from_document`.

The `verilator-parser` crate builds `ast::Design`, which owns its declarations, typed
expressions and statements, resolved variable/data-type IDs, parsed constants
and ranges, clock/process information, and source provenance. The `ast` module
contains neither JSON values nor FSM values, making it the stable boundary
between Verilator parsing and AIGER lowering. A future serializer can use the
retained identifiers, literal spellings, ranges, node kinds, and locations to
emit an equivalent supported JSON tree.

For compatibility, `convert::NamedFsm::try_from(&document)` still performs both
phases in one call when the parsed design has exactly one sensitivity domain.
the AIGER writers serialize the resulting ordered FSM.

Rust Simulator Generation
-------------------------

Generate a standalone Rust 2024 library from the same typed AST:

```sh
cargo run -p verilator-rust -- \
  tests/counter/build/ast.json \
  --clock clk \
  --output /tmp/generated-counter \
  --crate-name generated-counter

cargo test --manifest-path /tmp/generated-counter/Cargo.toml
```

Prefix the clock with `!` to select a negative edge. `--reset [!]NAME` permits
that reset edge in the sensitivity tree but does not apply reset automatically;
reset remains a field in the generated `Inputs` structure. The output directory
must be new or empty. The generator calculates a relative Cargo path from the
output project to `verilator-rust/verilator-rust-runtime`; the runtime itself has
no third-party dependencies.

Generated libraries expose `Model`, `Inputs`, `Outputs`, `State`, and
`Evaluation`. `Model::eval` observes combinational outputs and properties
without changing state. `Model::tick` captures pre-edge sampled values, executes
clocked processes,
commits pending nonblocking updates, and returns the post-edge evaluation:

```rust
use generated_counter::{Inputs, Model};

let mut model = Model::new();
let reset = Inputs {
    reset_n: false,
    enable: false,
};
assert_eq!(model.tick(&reset).outputs.count, 0);
```

Generated data types are written to `src/types.rs`; the `Inputs`, `Outputs`,
and `State` definitions and trait implementations are written to
`src/input.rs`, `src/output.rs`, and `src/state.rs`. The model implementation
and root re-exports remain in `src/lib.rs`.

Simulation is deterministic and two-state. Uninitialized values start at zero,
then retained static initializers and `initial` statements execute once. One-bit ports use `bool`, ports
through 128 bits use the smallest fitting unsigned integer, and wider ports use
`Bits<WIDTH>`. Generated public wrappers preserve named aliases, enums, packed
structs/unions, packed arrays, and unpacked array layouts. Anonymous types receive
unique generated Rust names. Assertions report
`true` when they hold; assumptions and covers report their active expression.
Signals with a dense trailing Verilator suffix from `_v0` through `_vN` and a
common value type are exposed as one Rust array field with the suffix removed.

Unsupported constructs produce conversion errors with the Verilator node type
and source location. The initial boundary excludes multiple or asynchronous
clock domains, unsupported partial writes, combinational cycles, and multiple
overlapping drivers.

AIGER Export
------------

Before the CLI writes, covers are removed, gate outputs are normalized, and
ordering is verified. `verilator-utils` then writes ASCII or binary AIGER 1.9 syntax.
With `--zero-init`, every latch reset value is set to zero before serialization.
Design outputs, assertions as bad-state properties, and assumptions as
constraints are retained, with AIGER symbols emitted from the FSM labels.
The CLI performs these preparation steps before dispatching by extension. FSM
variables are ordered as inputs, register bits, then topologically ordered
gates, so every gate depends only on inputs, latches, or earlier gates.

Code Style
----------

- Avoid using .iter(), always use .into_iter() (with borrowing if necessary) instead
- Within reason, leverage traits for multiple functions that implement similar behavior
- Avoid using matches!() and instead use == (just a matter of personal preference, fallback on a match statement if needed)
- Avoid named functions inside of other functions (just put them outside, its way less indenting), closures are fine to put inside other functions of course
- Always "use std::..." at the top, followed by a separate block for libraries, and then finally a single nested use statement for "use crate::{...}"
- Avoid `crate::something::MyStruct` when not in a use statement
- Avoid `pub use ...`
- Put `mod ...` before any `use ...`
