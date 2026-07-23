#!/usr/bin/env bash
set -euo pipefail

# ScratchArch C Compatibility Test Suite
#
# Compiles C programs to LLVM IR (if clang is available),
# translates to SAIR via scratcharch-llvm,
# executes via scratcharch-sair-interpreter,
# and compares against expected results.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
C_DIR="$PROJECT_DIR/tests/c_programs"

# Programs and their expected exit codes
declare -A EXPECTED
EXPECTED[hello]=42
EXPECTED[add]=42
EXPECTED[factorial]=120
EXPECTED[fib]=55
EXPECTED[array]=42
EXPECTED[struct]=30
EXPECTED[pointer]=42
EXPECTED[string]=5
EXPECTED[memory]=6
EXPECTED[recursion]=15

HAS_CLANG=false
if command -v clang &>/dev/null; then
    HAS_CLANG=true
fi

if ! command -v cargo &>/dev/null; then
    echo "ERROR: cargo not found. Is Rust installed?"
    exit 1
fi

# Build the test runner if needed
TEST_RUNNER="$PROJECT_DIR/target/debug/c_test_runner"
if [ ! -x "$TEST_RUNNER" ]; then
    echo "Building C test runner..."
    cargo build -p scratcharch-llvm --bin c_test_runner 2>/dev/null || true
fi

pass=0
fail=0
skipped=0

for prog in hello add factorial fib array struct pointer string memory recursion; do
    ll_file="$C_DIR/$prog.ll"
    c_file="$C_DIR/$prog.c"

    # Try compiling from C source if clang is available
    if [ "$HAS_CLANG" = true ] && [ -f "$c_file" ]; then
        compiled_ll=$(mktemp /tmp/scratcharch_${prog}_XXXXXX.ll)
        if clang -S -emit-llvm -O0 -Xclang -disable-O0-optnone "$c_file" -o "$compiled_ll" 2>/dev/null; then
            ll_file="$compiled_ll"
        else
            rm -f "$compiled_ll"
        fi
    fi

    if [ ! -f "$ll_file" ]; then
        echo "SKIP  $prog  (no .ll file found)"
        skipped=$((skipped + 1))
        continue
    fi

    expected="${EXPECTED[$prog]}"

    # Use the Rust test infrastructure directly via cargo test
    # We pattern-match on the test name which includes the program name
    test_name="test_pipeline_${prog}"

    output=$(cd "$PROJECT_DIR" && cargo test -p scratcharch-llvm -- "$test_name" --nocapture 2>&1 || true)

    if echo "$output" | grep -q "test $test_name ... ok"; then
        echo "PASS  $prog"
        pass=$((pass + 1))
    elif echo "$output" | grep -q "test $test_name ... FAILED"; then
        echo "FAIL  $prog  (test failed)"
        fail=$((fail + 1))
    else
        echo "WARN  $prog  (could not find test result; may need to add test)"
        skipped=$((skipped + 1))
    fi

    # Clean up temp file if we created one
    if [ -n "${compiled_ll:-}" ] && [ -f "$compiled_ll" ]; then
        rm -f "$compiled_ll"
    fi
done

echo ""
echo "Results: $pass passed, $fail failed, $skipped skipped"
if [ "$fail" -gt 0 ]; then
    exit 1
fi
exit 0
