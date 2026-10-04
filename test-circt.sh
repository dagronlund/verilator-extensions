#!/usr/bin/env bash
# Compile the listed SystemVerilog fixtures using CIRCT from PATH (Bash 3.2+).
set -o pipefail

usage() {
  cat <<'HELP'
Usage: test-circt.sh [options] [fixture ...]
  --bound N          BMC cycles (default: 10)
  --timeout N        Seconds per command (default: 60)
  --output-dir PATH  Artifacts (default: tests/circt-results/build)
  --shared-libs LIBS BMC libraries (default: CIRCT_BMC_SHARED_LIBS or detected Z3)
  --color MODE       auto, always, or never (default: auto)
  -h, --help         Show this help
Requires Bash 3.2+, circt-verilog, circt-opt, circt-bmc, and GNU timeout (or gtimeout).
HELP
}
die() { printf 'Error: %s\n' "$*" >&2; exit 2; }
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P) || exit 2
# Add new fixtures here; most use tb.sv with a top matching the fixture name.
fixtures=(
  case_statements combinational_loops counter counter_free counter_ones
  data_types fifo_stage fifo_stage_bypass fifo_stage_fail_assert
  fifo_stage_fail_free stream_stage multidim_arrays nba_semantics
  package_properties packet_switch public_submodules signed_operations wire_ports
)
fixture_inputs() {
  top=$name
  sources=(tb.sv)
  inputs=()
  case $name in
    fifo_stage|fifo_stage_fail_assert|fifo_stage_fail_free)
      top=fifo_stage_tb; sources+=(../lib/stage.sv ../lib/fifo.sv) ;;
    fifo_stage_bypass)
      top=fifo_stage_bypass_tb; sources+=(../lib/stage.sv ../lib/fifo.sv) ;;
    packet_switch) top=packet_switch_tb ;;
    stream_stage)
      top=stream_stage_formal
      sources=(std/std_pkg.sv stream/stream_pkg.sv stream/stream_intf.sv
               std/std_register.sv stream/stream_stage.sv tb.sv) ;;
  esac
  inputs+=("${sources[@]}")
}

bound=10 limit=60 output="$root/tests/circt-results/build" color=auto
libs=${CIRCT_BMC_SHARED_LIBS:-}
selected=()
while (( $# )); do
  case $1 in
    -h|--help) usage; exit 0 ;;
    --bound|--timeout|--output-dir|--shared-libs|--color)
      (( $# >= 2 )) || die "missing value for $1"
      case $1 in
        --bound) bound=$2 ;; --timeout) limit=$2 ;; --output-dir) output=$2 ;;
        --shared-libs) libs=$2 ;; --color) color=$2 ;;
      esac
      shift 2 ;;
    --*) die "unknown option: $1" ;;
    *) selected+=("$1"); shift ;;
  esac
done
[[ $bound =~ ^[1-9][0-9]*$ && $limit =~ ^[1-9][0-9]*$ ]] || die 'bound and timeout must be positive integers'
case $color in auto|always|never) ;; *) die 'color must be auto, always, or never' ;; esac
frontend=$(command -v circt-verilog) || die 'circt-verilog missing from PATH'
optimizer=$(command -v circt-opt) || die 'circt-opt missing from PATH'
bmc=$(command -v circt-bmc) || die 'circt-bmc missing from PATH'
timer=$(command -v timeout || command -v gtimeout) || die 'install GNU coreutils for timeout/gtimeout'
# Resolve tool paths before changing into fixture directories.
frontend=$(cd -- "$(dirname -- "$frontend")" && printf '%s/%s' "$PWD" "$(basename -- "$frontend")")
optimizer=$(cd -- "$(dirname -- "$optimizer")" && printf '%s/%s' "$PWD" "$(basename -- "$optimizer")")
bmc=$(cd -- "$(dirname -- "$bmc")" && printf '%s/%s' "$PWD" "$(basename -- "$bmc")")
timer=$(cd -- "$(dirname -- "$timer")" && printf '%s/%s' "$PWD" "$(basename -- "$timer")")
for name in "${selected[@]}"; do
  known=0
  for entry in "${fixtures[@]}"; do [[ $entry != "$name" ]] || known=1; done
  (( known )) || die "unknown fixture: $name"
done
if [[ -z $libs ]]; then
  for candidate in /opt/homebrew/lib/libz3.dylib /usr/local/lib/libz3.dylib \
      /usr/local/lib/libz3.so /usr/lib/libz3.so /usr/lib/*/libz3.so /usr/lib/*/libz3.so.4; do
    if [[ -f $candidate ]]; then libs=$candidate; break; fi
  done
fi
mkdir -p -- "$output" || exit 2
output=$(cd -- "$output" && pwd -P) || exit 2
green='' red='' yellow='' magenta='' reset=''
if [[ $color == always || ( $color == auto && -t 1 && ${NO_COLOR+x} != x ) ]]; then
  green=$'\033[32m'; red=$'\033[31m'; yellow=$'\033[33m'; magenta=$'\033[35m'; reset=$'\033[0m'
fi

canonical_source() {
  local file=$1
  (cd -- "$(dirname -- "$file")" && printf '%s/%s\n' "$PWD" "$(basename -- "$file")")
}
run_command() {
  { printf 'cwd: %s\n$ ' "$PWD"; printf '%q ' "$@"; printf '\n\n'; } > "$log"
  "$timer" -k 2 "$limit" "$@" >> "$log" 2>&1
  code=$?
  reason=''
  if (( code == 124 || code == 137 )); then reason="timeout after ${limit}s"
  elif (( code != 0 )); then reason="exit $code"; fi
}

printf 'CIRCT optimizer: %s\n' "$optimizer"
printf 'CIRCT frontend: %s\nCIRCT BMC:      %s\nBMC bound:     %s cycles; CIRCT testbench reset assumptions enabled\nLogs and IR:   %s\n' "$frontend" "$bmc" "$bound" "$output"
{ "$timer" -k 2 "$limit" "$frontend" --version; "$timer" -k 2 "$limit" "$optimizer" --version; "$timer" -k 2 "$limit" "$bmc" --version; } > "$output/versions.txt" 2>&1
printf 'fixture\tstage\tresult\texit_code\treason\tlog\n' > "$output/results.tsv"
: > "$output/covered-sources.txt"
: > "$output/uncovered-sources.txt"
stages=(moore llhd hw opt-two-valued opt-formal-core bmc)
counts=(0 0 0 0 0 0); skips=(0 0 0 0 0 0); cexts=(0 0 0 0 0 0); total=0 failed=0
shopt -s nullglob
for name in "${fixtures[@]}"; do
  fixture="$root/tests/$name"
  if (( ${#selected[@]} )); then
    wanted=0
    for selection in "${selected[@]}"; do [[ $selection != "$name" ]] || wanted=1; done
    (( wanted )) || continue
  fi
  ((total++)); dest="$output/$name"; mkdir -p -- "$dest" || exit 2
  cd -- "$fixture" || exit 2
  printf '\n%s\n' "$name"
  fixture_inputs
  for source in "${sources[@]}"; do
    canonical_source "$source" >> "$output/covered-sources.txt" || exit 2
  done
  hw_compiled=0; two_valued=0; formal_core=0; stage_index=0
  for stage in "${stages[@]}"; do
    log="$dest/$stage.log"; artifact="$dest/$stage.mlir"; code='-'; reason=''
    [[ $stage == bmc ]] || rm -f -- "$artifact"
    case $stage in
      opt-two-valued) (( hw_compiled )) || reason='blocked: HW compilation failed' ;;
      opt-formal-core) (( two_valued )) || reason='blocked: two-valued lowering failed' ;;
      bmc) (( formal_core )) || reason='blocked: formal lowering failed' ;;
    esac
    if [[ -z $reason ]]; then
      if [[ $stage == bmc ]]; then
        command=("$bmc" "$dest/opt-formal-core.mlir" "--module=$top" -b "$bound" --flatten-modules --async-reset-as-sync)
        [[ -z $libs ]] || command+=("--shared-libs=$libs")
      elif [[ $stage == opt-two-valued ]]; then
        command=("$optimizer" "$dest/hw.mlir" --comb-assume-two-valued -o "$artifact")
      elif [[ $stage == opt-formal-core ]]; then
        command=("$optimizer" "$dest/opt-two-valued.mlir" --lower-llhd-formal-to-core -o "$artifact")
      else
        command=("$frontend" "${inputs[@]}" -DCIRCT "--top=$top" "--ir-$stage" --detect-memories=false -o "$artifact")
      fi
      run_command "${command[@]}"
      if [[ $code == 0 ]]; then
        if [[ $stage != bmc ]]; then
          if [[ ! -f $artifact ]]; then reason='command produced no IR file'
          else
            case $stage in
              hw) hw_compiled=1 ;;
              opt-two-valued) two_valued=1 ;;
              opt-formal-core) formal_core=1 ;;
            esac
          fi
        elif grep -q 'no property provided' "$log"; then reason='no properties remain to check'
        elif grep -q 'Assertion can be violated' "$log"; then reason='assertion violation / counterexample'
        elif ! grep -q 'Bound reached with no violations' "$log"; then reason='no successful bounded-check result reported'; fi
      fi
    else
      printf '%s\n' "$reason" > "$log"
    fi
    if [[ $code == 0 && -z $reason ]]; then
      result=PASS; tint=$green; counts[stage_index]=$((counts[stage_index]+1)); reason=compiled
      case $stage in
        opt-*) reason='lowered' ;;
        bmc) reason='bounded check completed' ;;
      esac
    elif [[ $stage == bmc && $code == 0 && $reason == 'no properties remain to check' ]]; then
      result=SKIP; tint=$yellow; skips[stage_index]=$((skips[stage_index]+1))
    elif [[ $stage == bmc && $code == 0 && $reason == 'assertion violation / counterexample' ]]; then
      result=CEXT; tint=$magenta; cexts[stage_index]=$((cexts[stage_index]+1))
    else result=FAIL; tint=$red; failed=1; fi
    printf '\nRunner: %s: %s\n' "$result" "$reason" >> "$log"
    printf '  %s%s%s  %-15s  %s\n' "$tint" "$result" "$reset" "$stage" "$reason"
    [[ $result != FAIL && $result != CEXT ]] || printf '        %s\n' "$log"
    printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$name" "$stage" "$result" "$code" "$reason" "$log" >> "$output/results.tsv"
    ((stage_index++))
  done
done
(( total )) || die 'no fixtures selected'
sort -u "$output/covered-sources.txt" -o "$output/covered-sources.txt"
if (( ${#selected[@]} == 0 )); then
  while IFS= read -r -d '' source; do
    case $source in "$output"/*|*/build/*|*/build-*/*) continue ;; esac
    source=$(canonical_source "$source") || exit 2
    if ! grep -Fxq -- "$source" "$output/covered-sources.txt"; then
      printf '%s\n' "$source" >> "$output/uncovered-sources.txt"; failed=1
      printf '%sFAIL%s  uncovered HDL source: %s\n' "$red" "$reset" "$source"
    fi
  done < <(find "$root/tests" -type f \( -name '*.sv' -o -name '*.v' \) -print0)
fi
printf '\nSummary:\n'
index=0
for stage in "${stages[@]}"; do
  printf '  %-15s: %s/%s passed, %s skipped, %s counterexamples\n' "$stage" "${counts[index]}" "$total" "${skips[index]}" "${cexts[index]}"; ((index++))
done
printf 'Details: %s/results.tsv\n' "$output"
exit "$failed"
