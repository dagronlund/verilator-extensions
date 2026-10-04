# Test the SystemVerilog fixtures with CIRCT

```sh
~/verilator-extensions/test-circt.sh
```

The script contains an explicit list of 18 fixtures, their top modules, and source
files. It does not read build files or require Ninja. Add new fixtures to the
`fixtures` array and any special sources/options to `fixture_inputs`.

Requires Bash 3.2+, `circt-verilog`, `circt-opt`, and `circt-bmc` on PATH, and GNU `timeout` or
`gtimeout` (on macOS: `brew install coreutils`). No Python is needed.

Each fixture runs these stages in order:

1. SystemVerilog → Moore: `circt-verilog --ir-moore`.
2. SystemVerilog → LLHD: `circt-verilog --ir-llhd`.
3. SystemVerilog → HW: `circt-verilog --ir-hw`.
4. HW → two-valued IR: `circt-opt --comb-assume-two-valued`.
5. Two-valued IR → formal core IR: `circt-opt --lower-llhd-formal-to-core`.
6. Formal core IR → BMC: `circt-bmc --flatten-modules --async-reset-as-sync -b 10`.

All frontend commands pass `-DCIRCT` and use `--detect-memories=false` to keep memories as register
arrays that BMC can lower. The frontend stages run independently. A stage passes when the compiler exits
successfully and produces an IR file; the script does not search IR for dialects.
The two optimizer phases run as separate commands, each consuming the preceding
phase’s output. BMC consumes `opt-formal-core.mlir`. Each dependent phase reports
a blocked failure if its input phase fails. Every phase has its own status, log,
and summary count. The two-valued phase assumes binary logic and discards X/Z
distinctions.
BMC reports PASS for a bound-reached result with no violations, CEXT for a
counterexample, and SKIP for successful runs with no properties. Top-level testbenches with active-low `reset_n` use a `CIRCT`-guarded
assumption to assert reset at the first rising edge and deassert it at later
edges. The stream-stage fixture retains its existing active-high reset assumption.
Deliberately failing fixtures are not special-cased.

`stream_stage` contains local copies of the Reptilia RTL, specialized
to the synthesis-only branches. Static-assert macros and their header are removed;
no synthesis-mode define or sibling checkout is needed. Gecko is excluded from the CIRCT fixture list.
FIFO fixtures include the shared sources from `tests/lib`.

PASS is green, FAIL is red, SKIP is yellow, and CEXT is magenta in a terminal.
Skips and counterexamples are counted separately and do not cause a nonzero exit.
CEXT does not distinguish expected counterexamples from unexpected ones; inspect
the fixture’s BMC log for the trace. The script continues after failures
and exits 1 if any check fails or any HDL source under `tests` is uncovered by the
fixture list. Setup errors exit 2. Logs, IR, tool versions, `results.tsv`, and
source coverage lists are saved in `tests/circt-results/build/`. Each run replaces
the summary and selected fixtures' logs. This directory is already ignored by Git.

```sh
./test-circt.sh counter fifo_stage --bound 20 --timeout 120
./test-circt.sh --color always
./test-circt.sh --shared-libs /path/to/libz3.so
PATH=/path/to/circt/build/bin:$PATH ./test-circt.sh
```

`--shared-libs` defaults to `CIRCT_BMC_SHARED_LIBS`, then common Z3 library paths.
Use `--output-dir PATH` for a different artifact directory. `--color never` or
`NO_COLOR=1` disables automatic color. See `--help` for all options.
