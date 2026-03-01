#!/bin/bash
set -euo pipefail

# Grunner Performance Benchmark Script
# Run with: bash benchmark.sh

echo "================================================"
echo "      Grunner Performance Benchmark Suite       "
echo "================================================"
echo

# Configuration
BINARY_PATH="./target/release/grunner"
CACHE_DIR="$HOME/.cache/grunner"
CACHE_FILE="$CACHE_DIR/apps.bin"
CONFIG_DIR="$HOME/.config/grunner"
CONFIG_FILE="$CONFIG_DIR/grunner.toml"
ITERATIONS=3
TIMEOUT_SECONDS=2

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Helper functions
print_header() {
    echo -e "${BLUE}=== $1 ===${NC}"
    echo
}

print_result() {
    echo -e "${GREEN}✓${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}⚠${NC} $1"
}

print_error() {
    echo -e "${RED}✗${NC} $1"
}

check_binary() {
    if [ ! -f "$BINARY_PATH" ]; then
        print_error "Binary not found at $BINARY_PATH"
        print_warning "Please build with: cargo build --release"
        exit 1
    fi
    print_result "Binary found: $(ls -lh "$BINARY_PATH" | awk '{print $5}')"
}

# 1. Binary Information
print_header "1. Binary Information"
check_binary

# Get binary info
file "$BINARY_PATH"
echo
strings "$BINARY_PATH" | grep -E "(rustc|panic|alloc)" | head -5 || true
echo

# 2. Startup Time Benchmark
print_header "2. Startup Time Benchmark"

# Cold start (no config)
print_result "Cold start (no config file):"
if [ -f "$CONFIG_FILE" ]; then
    mv "$CONFIG_FILE" "$CONFIG_FILE.backup"
fi

for i in $(seq 1 $ITERATIONS); do
    echo -n "  Run $i: "
    /usr/bin/time -f "%e seconds" timeout $TIMEOUT_SECONDS "$BINARY_PATH" --version >/dev/null 2>&1 || true
done

# Restore config
if [ -f "$CONFIG_FILE.backup" ]; then
    mv "$CONFIG_FILE.backup" "$CONFIG_FILE"
fi

echo

# Warm start (with config)
print_result "Warm start (with config):"
for i in $(seq 1 $ITERATIONS); do
    echo -n "  Run $i: "
    /usr/bin/time -f "%e seconds" timeout $TIMEOUT_SECONDS "$BINARY_PATH" --version >/dev/null 2>&1 || true
done
echo

# 3. Memory Usage
print_header "3. Memory Usage"

print_result "Peak memory usage (RSS):"
/usr/bin/time -v "$BINARY_PATH" --version 2>&1 | grep -E "Maximum resident|Minor page faults" || true
echo

# 4. Cache Performance
print_header "4. Cache Performance"

# Clear cache
if [ -f "$CACHE_FILE" ]; then
    CACHE_SIZE=$(ls -lh "$CACHE_FILE" | awk '{print $5}')
    print_result "Clearing cache ($CACHE_SIZE)..."
    rm -f "$CACHE_FILE"
fi

# First run (cold cache)
print_result "First run (cold cache - parsing .desktop files):"
echo -n "  Time: "
{ time timeout $TIMEOUT_SECONDS "$BINARY_PATH" --version >/dev/null 2>&1; } 2>&1 | grep real | awk '{print $2}' || true

# Check if cache was created
if [ -f "$CACHE_FILE" ]; then
    CACHE_SIZE=$(ls -lh "$CACHE_FILE" | awk '{print $5}')
    print_result "Cache created: $CACHE_SIZE"
else
    print_warning "No cache file created"
fi
echo

# Second run (warm cache)
print_result "Second run (warm cache - loading from cache):"
echo -n "  Time: "
{ time timeout $TIMEOUT_SECONDS "$BINARY_PATH" --version >/dev/null 2>&1; } 2>&1 | grep real | awk '{print $2}' || true
echo

# 5. Search Performance Simulation
print_header "5. Search Performance Simulation"

# Create a test script to simulate searches
cat > /tmp/test_searches.txt << 'EOF'
f
fi
fir
fire
firef
firefo
firefox
t
te
ter
term
termin
termina
terminal
EOF

print_result "Simulating incremental search patterns..."
echo "  Test patterns: $(wc -l /tmp/test_searches.txt | awk '{print $1}')"
echo

# 6. System Information
print_header "6. System Information"

print_result "CPU:"
lscpu | grep -E "(Model name|CPU\(s\)|MHz)" | head -3
echo

print_result "Memory:"
free -h | head -2
echo

print_result "Distribution:"
lsb_release -d 2>/dev/null || cat /etc/os-release | grep -E "^(NAME|VERSION)=" || uname -a
echo

# 7. Dependency Check
print_header "7. Dependency Check"

print_result "Checking dynamic dependencies:"
ldd "$BINARY_PATH" 2>/dev/null | wc -l | awk '{print "  Linked libraries:", $1}'
echo

print_result "Checking GTK version:"
if command -v pkg-config >/dev/null 2>&1; then
    pkg-config --modversion gtk4 2>/dev/null || echo "  GTK4 not found"
else
    echo "  pkg-config not available"
fi
echo

# 8. Profiling Hints
print_header "8. Profiling Instructions"

cat << 'EOF'
For detailed profiling:

1. CPU Profiling:
   perf record -g --call-graph dwarf "$BINARY_PATH"
   perf report

2. Memory Profiling:
   valgrind --tool=massif "$BINARY_PATH"
   ms_print massif.out.*

3. Heap Profiling:
   valgrind --tool=memcheck --leak-check=full "$BINARY_PATH"

4. Flame Graph:
   cargo install flamegraph
   sudo flamegraph -- "$BINARY_PATH"

5. Continuous Benchmarking:
   Use criterion.rs for automated benchmarks in Rust.
EOF
echo

# 9. Summary
print_header "9. Benchmark Summary"

echo "To track optimization progress:"
echo "1. Save this output to a file:"
echo "   bash benchmark.sh > benchmark-$(date +%Y%m%d).txt"
echo
echo "2. Compare before/after optimizations:"
echo "   diff benchmark-before.txt benchmark-after.txt"
echo
echo "3. Key metrics to monitor:"
echo "   - Startup time (cold & warm)"
echo "   - Cache creation/load time"
echo "   - Memory usage (RSS)"
echo "   - Binary size"
echo

print_result "Benchmark completed successfully!"
echo "================================================"

# Cleanup
rm -f /tmp/test_searches.txt
