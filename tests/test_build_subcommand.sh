#!/bin/bash
# Integration tests for m2c build/run/test/clean subcommands

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
M2C="${M2C:-$SCRIPT_DIR/target/debug/m2c}"
TMPDIR=$(mktemp -d /tmp/m2c_build_test.XXXXXX)
PASS=0
FAIL=0

cleanup() {
    rm -rf "$TMPDIR"
}
trap cleanup EXIT

pass() {
    PASS=$((PASS + 1))
    echo "  PASS: $1"
}

fail() {
    FAIL=$((FAIL + 1))
    echo "  FAIL: $1"
}

# Create a minimal project
setup_project() {
    local dir="$1"
    mkdir -p "$dir/src"

    cat > "$dir/m2.toml" << 'MANIFEST'
name=testproj
version=0.1.0
entry=src/Main.mod
includes=src
MANIFEST

    cat > "$dir/src/Main.mod" << 'M2SRC'
MODULE Main;
FROM InOut IMPORT WriteString, WriteLn;
BEGIN
  WriteString("hello from build");
  WriteLn;
END Main.
M2SRC
}

# Create a project with test entry
setup_project_with_tests() {
    local dir="$1"
    setup_project "$dir"

    mkdir -p "$dir/tests"

    cat >> "$dir/m2.toml" << 'MANIFEST'

[test]
entry=tests/TestMain.mod
includes=tests
MANIFEST

    cat > "$dir/tests/TestMain.mod" << 'M2SRC'
MODULE TestMain;
FROM InOut IMPORT WriteString, WriteLn;
BEGIN
  WriteString("test passed");
  WriteLn;
END TestMain.
M2SRC
}

echo "=== m2c build subcommand tests ==="

# Test 1: m2c build creates artifact
echo "Test 1: m2c build"
PROJ1="$TMPDIR/proj1"
setup_project "$PROJ1"
cd "$PROJ1"
if $M2C build 2>&1; then
    if [ -f ".m2c/bin/testproj" ]; then
        pass "m2c build creates .m2c/bin/testproj"
    else
        fail "m2c build did not create .m2c/bin/testproj"
    fi
else
    fail "m2c build exited with error"
fi

# Test 2: m2c build again shows up to date
echo "Test 2: m2c build (up to date)"
cd "$PROJ1"
OUTPUT=$($M2C build 2>&1)
if echo "$OUTPUT" | grep -q "up to date"; then
    pass "m2c build shows 'up to date' on second run"
else
    fail "m2c build did not show 'up to date' (got: $OUTPUT)"
fi

# Test 3: touch source, rebuild
echo "Test 3: rebuild after source change"
cd "$PROJ1"
sleep 1
touch src/Main.mod
OUTPUT=$($M2C build 2>&1)
if echo "$OUTPUT" | grep -qv "up to date"; then
    pass "m2c build rebuilds after source change"
else
    fail "m2c build still shows 'up to date' after source change"
fi

# Test 4: m2c run executes the binary
echo "Test 4: m2c run"
PROJ2="$TMPDIR/proj2"
setup_project "$PROJ2"
cd "$PROJ2"
OUTPUT=$($M2C run 2>&1)
if echo "$OUTPUT" | grep -q "hello from build"; then
    pass "m2c run executes binary"
else
    fail "m2c run did not produce expected output (got: $OUTPUT)"
fi

# Test 5: m2c clean removes .m2c/
echo "Test 5: m2c clean"
cd "$PROJ2"
$M2C build 2>&1 >/dev/null
if [ -d ".m2c" ]; then
    $M2C clean 2>&1
    if [ ! -d ".m2c" ]; then
        pass "m2c clean removes .m2c/"
    else
        fail "m2c clean did not remove .m2c/"
    fi
else
    fail "m2c build did not create .m2c/ for clean test"
fi

# Test 6: m2c test builds and runs test entry
echo "Test 6: m2c test"
PROJ3="$TMPDIR/proj3"
setup_project_with_tests "$PROJ3"
cd "$PROJ3"
OUTPUT=$($M2C test 2>&1)
if echo "$OUTPUT" | grep -q "test passed"; then
    pass "m2c test runs test entry"
else
    fail "m2c test did not produce expected output (got: $OUTPUT)"
fi

# Test 7: m2c build with --release flag
echo "Test 7: m2c build --release"
PROJ4="$TMPDIR/proj4"
setup_project "$PROJ4"
cd "$PROJ4"
if $M2C build --release 2>&1; then
    if [ -f ".m2c/bin/testproj" ]; then
        pass "m2c build --release creates artifact"
    else
        fail "m2c build --release did not create artifact"
    fi
else
    fail "m2c build --release exited with error"
fi

# Test 8: m2c build with no m2.toml
echo "Test 8: m2c build with no manifest"
NOPROJ="$TMPDIR/noproj"
mkdir -p "$NOPROJ"
cd "$NOPROJ"
if $M2C build 2>&1; then
    fail "m2c build should fail without m2.toml"
else
    pass "m2c build fails without m2.toml"
fi

echo ""
echo "Results: $PASS passed, $FAIL failed"
if [ $FAIL -gt 0 ]; then
    exit 1
fi
