#!/bin/bash
# Integration tests for mx build/run/test/clean subcommands

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
MX="${MX:-$SCRIPT_DIR/target/debug/mx}"
TMPDIR=$(mktemp -d /tmp/mx_build_test.XXXXXX)
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

echo "=== mx build subcommand tests ==="

# Test 1: mx build creates artifact
echo "Test 1: mx build"
PROJ1="$TMPDIR/proj1"
setup_project "$PROJ1"
cd "$PROJ1"
if $MX build 2>&1; then
    if [ -f ".mx/bin/testproj" ]; then
        pass "mx build creates .mx/bin/testproj"
    else
        fail "mx build did not create .mx/bin/testproj"
    fi
else
    fail "mx build exited with error"
fi

# Test 2: mx build again shows up to date
echo "Test 2: mx build (up to date)"
cd "$PROJ1"
OUTPUT=$($MX build 2>&1)
if echo "$OUTPUT" | grep -q "up to date"; then
    pass "mx build shows 'up to date' on second run"
else
    fail "mx build did not show 'up to date' (got: $OUTPUT)"
fi

# Test 3: touch source, rebuild
echo "Test 3: rebuild after source change"
cd "$PROJ1"
sleep 1
touch src/Main.mod
OUTPUT=$($MX build 2>&1)
if echo "$OUTPUT" | grep -qv "up to date"; then
    pass "mx build rebuilds after source change"
else
    fail "mx build still shows 'up to date' after source change"
fi

# Test 4: mx run executes the binary
echo "Test 4: mx run"
PROJ2="$TMPDIR/proj2"
setup_project "$PROJ2"
cd "$PROJ2"
OUTPUT=$($MX run 2>&1)
if echo "$OUTPUT" | grep -q "hello from build"; then
    pass "mx run executes binary"
else
    fail "mx run did not produce expected output (got: $OUTPUT)"
fi

# Test 5: mx clean removes .mx/
echo "Test 5: mx clean"
cd "$PROJ2"
$MX build 2>&1 >/dev/null
if [ -d ".mx" ]; then
    $MX clean 2>&1
    if [ ! -d ".mx" ]; then
        pass "mx clean removes .mx/"
    else
        fail "mx clean did not remove .mx/"
    fi
else
    fail "mx build did not create .mx/ for clean test"
fi

# Test 6: mx test builds and runs test entry
echo "Test 6: mx test"
PROJ3="$TMPDIR/proj3"
setup_project_with_tests "$PROJ3"
cd "$PROJ3"
OUTPUT=$($MX test 2>&1)
if echo "$OUTPUT" | grep -q "test passed"; then
    pass "mx test runs test entry"
else
    fail "mx test did not produce expected output (got: $OUTPUT)"
fi

# Test 7: mx build with --release flag
echo "Test 7: mx build --release"
PROJ4="$TMPDIR/proj4"
setup_project "$PROJ4"
cd "$PROJ4"
if $MX build --release 2>&1; then
    if [ -f ".mx/bin/testproj" ]; then
        pass "mx build --release creates artifact"
    else
        fail "mx build --release did not create artifact"
    fi
else
    fail "mx build --release exited with error"
fi

# Test 8: mx build with no m2.toml
echo "Test 8: mx build with no manifest"
NOPROJ="$TMPDIR/noproj"
mkdir -p "$NOPROJ"
cd "$NOPROJ"
if $MX build 2>&1; then
    fail "mx build should fail without m2.toml"
else
    pass "mx build fails without m2.toml"
fi

echo ""
echo "Results: $PASS passed, $FAIL failed"
if [ $FAIL -gt 0 ]; then
    exit 1
fi
