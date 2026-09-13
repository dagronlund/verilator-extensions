#!/bin/sh

set -eu

tests_directory=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repository_directory=$(dirname -- "$tests_directory")
binary="$repository_directory/target/debug/verilator-formal"

(
    cd "$repository_directory"
    cargo build
)

for build_file in "$tests_directory"/*/build.ninja; do
    fixture_directory=$(dirname -- "$build_file")
    ast="$fixture_directory/build/ast.json"
    output="$fixture_directory/build/test.aig"
    ron_output="$fixture_directory/build/test.ron"

    rm -rf "$fixture_directory/build"
    ninja -C "$fixture_directory"
    if [ "$(basename -- "$fixture_directory")" = gecko_core ]; then
        echo "Running $binary $ast --clock clk --ron-output $ron_output"
        "$binary" "$ast" --clock clk --ron-output "$ron_output"
    else
        echo "Running $binary $ast --clock clk --reset '!reset_n' --output $output --ron-output $ron_output"
        "$binary" "$ast" --clock clk --reset '!reset_n' --output "$output" --ron-output "$ron_output"
    fi
done
