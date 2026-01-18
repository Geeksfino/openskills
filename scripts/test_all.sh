#!/usr/bin/env bash
# Run all tests for OpenSkills Runtime
# This script runs tests for Rust runtime, TypeScript bindings, and Python bindings

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Track test results
RUST_TESTS_PASSED=true
TS_TESTS_PASSED=true
PY_TESTS_PASSED=true

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}OpenSkills Runtime - Test Suite${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# 1. Run Rust tests
echo -e "${BLUE}[1/3] Running Rust runtime tests...${NC}"
if cargo test --all 2>&1 | tee /tmp/rust_tests.log; then
    echo -e "${GREEN}✓ Rust tests passed${NC}"
else
    echo -e "${RED}✗ Rust tests failed${NC}"
    RUST_TESTS_PASSED=false
fi
echo ""

# 2. Run TypeScript binding tests
echo -e "${BLUE}[2/3] Running TypeScript binding tests...${NC}"
cd "$ROOT/bindings/ts"
if npm test 2>&1 | tee /tmp/ts_tests.log; then
    echo -e "${GREEN}✓ TypeScript tests passed${NC}"
else
    echo -e "${RED}✗ TypeScript tests failed${NC}"
    TS_TESTS_PASSED=false
fi
cd "$ROOT"
echo ""

# 3. Run Python binding tests
echo -e "${BLUE}[3/3] Running Python binding tests...${NC}"
cd "$ROOT/bindings/python"

# Check if virtual environment exists, create if not
if [ ! -d "test-env" ]; then
    echo -e "${YELLOW}Creating Python virtual environment...${NC}"
    python3 -m venv test-env
    source test-env/bin/activate
    pip install -q pytest
    pip install -q "$ROOT/target/wheels/"*.whl 2>/dev/null || {
        echo -e "${YELLOW}Building Python wheel first...${NC}"
        cd "$ROOT"
        "$ROOT/scripts/build_python.sh"
        cd "$ROOT/bindings/python"
        pip install -q "$ROOT/target/wheels/"*.whl
    }
else
    source test-env/bin/activate
fi

if python3 -m pytest tests/test_runtime.py -v 2>&1 | tee /tmp/py_tests.log; then
    echo -e "${GREEN}✓ Python tests passed${NC}"
else
    echo -e "${RED}✗ Python tests failed${NC}"
    PY_TESTS_PASSED=false
fi
cd "$ROOT"
echo ""

# Summary
echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Test Summary${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

if [ "$RUST_TESTS_PASSED" = true ] && [ "$TS_TESTS_PASSED" = true ] && [ "$PY_TESTS_PASSED" = true ]; then
    echo -e "${GREEN}✓ All tests passed!${NC}"
    echo ""
    exit 0
else
    echo -e "${RED}✗ Some tests failed:${NC}"
    [ "$RUST_TESTS_PASSED" = false ] && echo -e "  ${RED}✗ Rust tests${NC}"
    [ "$TS_TESTS_PASSED" = false ] && echo -e "  ${RED}✗ TypeScript tests${NC}"
    [ "$PY_TESTS_PASSED" = false ] && echo -e "  ${RED}✗ Python tests${NC}"
    echo ""
    echo -e "${YELLOW}Test logs saved to:${NC}"
    [ "$RUST_TESTS_PASSED" = false ] && echo -e "  Rust: /tmp/rust_tests.log"
    [ "$TS_TESTS_PASSED" = false ] && echo -e "  TypeScript: /tmp/ts_tests.log"
    [ "$PY_TESTS_PASSED" = false ] && echo -e "  Python: /tmp/py_tests.log"
    echo ""
    exit 1
fi
