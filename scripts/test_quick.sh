#!/usr/bin/env bash
# Quick test script - runs tests without full rebuild
# Use test_all.sh for comprehensive regression testing

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}Running quick tests...${NC}"
echo ""

# Rust tests (fastest)
echo -e "${BLUE}[1/3] Rust tests...${NC}"
cargo test --lib --test skill_session_tests 2>&1 | grep -E "(test result|Running|passed|failed)" || true
echo ""

# TypeScript tests
echo -e "${BLUE}[2/3] TypeScript tests...${NC}"
cd bindings/ts && npm test 2>&1 | tail -5
cd "$ROOT"
echo ""

# Python tests
echo -e "${BLUE}[3/3] Python tests...${NC}"
cd bindings/python
if [ -d "test-env" ]; then
    source test-env/bin/activate
    python3 -m pytest tests/test_runtime.py -v 2>&1 | tail -10
else
    echo "Python test environment not set up. Run test_all.sh first."
fi
cd "$ROOT"

echo -e "${GREEN}Quick tests complete!${NC}"
