#!/usr/bin/env bash
# Compile the listed SystemVerilog fixtures and verify circt-formal conversion (Bash 3.2+).
set -o pipefail

usage() {
  cat <<'HELP'
Usage: test-circt.sh [options] [fixture ...]
  --timeout N        Seconds per command (default: 60)
  --output-dir PATH  Artifacts (default: tests/circt-results/build)
  --color MODE       auto, always, or never (default: auto)
  -h, --help         Show this help
Requires Bash 3.2+, Cargo, circt-verilog, circt-opt, and GNU timeout (or gtimeout).
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

limit=60 output="$root/tests/circt-results/build" color=auto
selected=()
while (( $# )); do
  case $1 in
    -h|--help) usage; exit 0 ;;
    --timeout|--output-dir|--color)
      (( $# >= 2 )) || die "missing value for $1"
      case $1 in
        --timeout) limit=$2 ;; --output-dir) output=$2 ;; --color) color=$2 ;;
      esac
      shift 2 ;;
    --*) die "unknown option: $1" ;;
    *) selected+=("$1"); shift ;;
  esac
done
[[ $limit =~ ^[1-9][0-9]*$ ]] || die 'timeout must be a positive integer'
case $color in auto|always|never) ;; *) die 'color must be auto, always, or never' ;; esac
frontend=$(command -v circt-verilog) || die 'circt-verilog missing from PATH'
optimizer=$(command -v circt-opt) || die 'circt-opt missing from PATH'
cargo=$(command -v cargo) || die 'cargo missing from PATH'
timer=$(command -v timeout || command -v gtimeout) || die 'install GNU coreutils for timeout/gtimeout'
# Resolve tool paths before changing into fixture directories.
frontend=$(cd -- "$(dirname -- "$frontend")" && printf '%s/%s' "$PWD" "$(basename -- "$frontend")")
optimizer=$(cd -- "$(dirname -- "$optimizer")" && printf '%s/%s' "$PWD" "$(basename -- "$optimizer")")
formal="$root/target/debug/circt-formal"
timer=$(cd -- "$(dirname -- "$timer")" && printf '%s/%s' "$PWD" "$(basename -- "$timer")")
for name in "${selected[@]}"; do
  known=0
  for entry in "${fixtures[@]}"; do [[ $entry != "$name" ]] || known=1; done
  (( known )) || die "unknown fixture: $name"
done
"$cargo" build --manifest-path "$root/Cargo.toml" --target-dir "$root/target" -p circt-formal || exit 2
mkdir -p -- "$output" || exit 2
output=$(cd -- "$output" && pwd -P) || exit 2
green='' red='' reset=''
if [[ $color == always || ( $color == auto && -t 1 && ${NO_COLOR+x} != x ) ]]; then
  green=$'\033[32m'; red=$'\033[31m'; reset=$'\033[0m'
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
printf 'CIRCT frontend: %s\ncirct-formal:   %s\nLogs and IR:   %s\n' "$frontend" "$formal" "$output"
{ "$timer" -k 2 "$limit" "$frontend" --version; "$timer" -k 2 "$limit" "$optimizer" --version; } > "$output/versions.txt" 2>&1
printf 'fixture\tstage\tresult\texit_code\treason\tlog\n' > "$output/results.tsv"
: > "$output/covered-sources.txt"
: > "$output/uncovered-sources.txt"
stages=(moore llhd hw opt-two-valued opt-formal-core formal)
counts=(0 0 0 0 0 0); total=0 failed=0
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
    [[ $stage == formal ]] || rm -f -- "$artifact"
    case $stage in
      opt-two-valued) (( hw_compiled )) || reason='blocked: HW compilation failed' ;;
      opt-formal-core) (( two_valued )) || reason='blocked: two-valued lowering failed' ;;
      formal) (( formal_core )) || reason='blocked: formal lowering failed' ;;
    esac
    if [[ -z $reason ]]; then
      if [[ $stage == formal ]]; then
        command=("$formal" "$dest/opt-formal-core.mlir" --top "$top" --clock clk)
      elif [[ $stage == opt-two-valued ]]; then
        command=("$optimizer" "$dest/hw.mlir" --comb-assume-two-valued -o "$artifact")
      elif [[ $stage == opt-formal-core ]]; then
        command=("$optimizer" "$dest/opt-two-valued.mlir" --lower-llhd-formal-to-core -o "$artifact")
      else
        command=("$frontend" "${inputs[@]}" -DCIRCT "--top=$top" "--ir-$stage" --detect-memories=false -o "$artifact")
      fi
      run_command "${command[@]}"
      if [[ $code == 0 ]]; then
        if [[ $stage != formal ]]; then
          if [[ ! -f $artifact ]]; then reason='command produced no IR file'
          else
            case $stage in
              hw) hw_compiled=1 ;;
              opt-two-valued) two_valued=1 ;;
              opt-formal-core) formal_core=1 ;;
            esac
          fi
        fi
      fi
    else
      printf '%s\n' "$reason" > "$log"
    fi
    if [[ $code == 0 && -z $reason ]]; then
      result=PASS; tint=$green; counts[stage_index]=$((counts[stage_index]+1)); reason=compiled
      case $stage in
        opt-*) reason='lowered' ;;
        formal) reason='conversion completed' ;;
      esac
    else result=FAIL; tint=$red; failed=1; fi
    printf '\nRunner: %s: %s\n' "$result" "$reason" >> "$log"
    printf '  %s%s%s  %-15s  %s\n' "$tint" "$result" "$reset" "$stage" "$reason"
    [[ $result != FAIL ]] || printf '        %s\n' "$log"
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
  printf '  %-15s: %s/%s passed\n' "$stage" "${counts[index]}" "$total"; ((index++))
done
printf 'Details: %s/results.tsv\n' "$output"
exit "$failed"
