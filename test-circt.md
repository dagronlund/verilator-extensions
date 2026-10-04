# Test the SystemVerilog fixtures with CIRCT

```sh
~/verilator-extensions/test-circt.sh
```

The script contains an explicit list of 18 fixtures, their top modules, and source
files. It does not read build files or require Ninja. Add new fixtures to the
`fixtures` array and any special sources/options to `fixture_inputs`.

Requires Bash 3.2+, Cargo, `circt-verilog`, and `circt-opt` on PATH, and GNU `timeout` or
`gtimeout` (on macOS: `brew install coreutils`). The script builds the workspace’s
`circt-formal` binary once before running the fixtures. No Python is needed.

Each fixture runs these stages in order:

1. SystemVerilog → Moore: `circt-verilog --ir-moore`.
2. SystemVerilog → LLHD: `circt-verilog --ir-llhd`.
3. SystemVerilog → HW: `circt-verilog --ir-hw`.
4. HW → two-valued IR: `circt-opt --comb-assume-two-valued`.
5. Two-valued IR → formal core IR: `circt-opt --lower-llhd-formal-to-core`.
6. Formal core IR → FSM conversion: `circt-formal opt-formal-core.mlir --top TOP --clock clk`.

All frontend commands pass `-DCIRCT` and use `--detect-memories=false` to keep memories as register
arrays that `circt-formal` can convert. The frontend stages run independently. A stage passes when the compiler exits
successfully and produces an IR file; the script does not search IR for dialects.
The two optimizer phases run as separate commands, each consuming the preceding
phase’s output. `circt-formal` consumes `opt-formal-core.mlir`. Each dependent phase reports
a blocked failure if its input phase fails. Every phase has its own status, log,
and summary count. The two-valued phase assumes binary logic and discards X/Z
distinctions.
The conversion stage passes when `circt-formal` exits successfully. It does not
run a model checker or inspect property outcomes. Top-level testbenches with active-low `reset_n` use a `CIRCT`-guarded
assumption to assert reset at the first rising edge and deassert it at later
edges. The stream-stage fixture retains its existing active-high reset assumption.
Deliberately failing fixtures are not special-cased.

`stream_stage` contains local copies of the Reptilia RTL, specialized
to the synthesis-only branches. Static-assert macros and their header are removed;
no synthesis-mode define or sibling checkout is needed. Gecko is excluded from the CIRCT fixture list.
FIFO fixtures include the shared sources from `tests/lib`.

PASS is green and FAIL is red in a terminal. The script continues after failures
and exits 1 if any check fails or any HDL source under `tests` is uncovered by the
fixture list. Setup errors exit 2. Logs, IR, tool versions, `results.tsv`, and
source coverage lists are saved in `tests/circt-results/build/`. Each run replaces
the summary and selected fixtures' logs. This directory is already ignored by Git.

```sh
./test-circt.sh counter fifo_stage --timeout 120
./test-circt.sh --color always
PATH=/path/to/circt/build/bin:$PATH ./test-circt.sh
```

Use `--output-dir PATH` for a different artifact directory. `--color never` or
`NO_COLOR=1` disables automatic color. See `--help` for all options.
