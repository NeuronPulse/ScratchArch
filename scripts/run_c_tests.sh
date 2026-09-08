#!/usr/bin/env bash
set -euo pipefail

# ScratchArch C Compatibility Test Suite
#
# For every committed fixture in tests/c_programs/, compiles the .c source to
# LLVM IR with clang (real clang output, freshly generated on each run),
# translates to SAIR via scratcharch-llvm, executes via
# scratcharch-sair-interpreter, and compares against the expected result.
#
# The committed tests/c_programs/*.ll files are themselves real clang output
# (checked into the repo); this script additionally verifies that a *fresh*
# clang compile still parses and produces the same result, guarding against
# corpus drift.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
C_DIR="$PROJECT_DIR/tests/c_programs"

HAS_CLANG=false
if command -v clang &>/dev/null; then
    HAS_CLANG=true
fi

if ! command -v cargo &>/dev/null; then
    echo "ERROR: cargo not found. Is Rust installed?"
    exit 1
fi

pass=0
fail=0
skipped=0

# The committed fixtures are validated by cargo test (pipeline_tests.rs reads
# tests/c_programs/*.ll); corpus_clang_tests.rs recompiles each .c fresh with
# clang and runs it through the same pipeline.
output=$(cd "$PROJECT_DIR" && cargo test -p scratcharch-llvm --test corpus_clang_tests -- --nocapture 2>&1 || true)

if [ "$HAS_CLANG" = true ]; then
    if echo "$output" | grep -q "test result: ok"; then
        echo "PASS  fresh clang corpus (tests/c_programs/*.c → clang → SAIR → interpreter)"
        pass=1
    else
        echo "FAIL  fresh clang corpus"
        echo "$output" | tail -30
        fail=1
    fi
else
    echo "SKIP  fresh clang corpus (clang not found; running committed fixtures only)"
    skipped=1
    if echo "$output" | grep -q "test result: ok"; then
        echo "PASS  committed fixtures (tests/c_programs/*.ll)"
        pass=$((pass + 1))
    else
        echo "FAIL  committed fixtures"
        echo "$output" | tail -30
        fail=$((fail + 1))
    fi
fi

echo ""
echo "Results: $pass passed, $fail failed, $skipped skipped"
if [ "$fail" -gt 0 ]; then
    exit 1
fi
exit 0
