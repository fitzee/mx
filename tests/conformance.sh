#!/bin/bash
# Conformance test — validates the mx toolchain contract.
# Requires: jq, cargo (or pre-built mx on PATH)
set -e

MX="${MX:-cargo run --quiet --}"
PASS=0
FAIL=0
TMPDIR=$(mktemp -d)
trap "rm -rf $TMPDIR" EXIT

pass() { echo "  PASS: $1"; PASS=$((PASS + 1)); }
fail() { echo "  FAIL: $1"; FAIL=$((FAIL + 1)); }

echo "=== mx conformance tests ==="

# 1. --version-json: parse with jq, assert required fields
echo "1. --version-json"
VJ=$($MX --version-json 2>&1)
if echo "$VJ" | jq -e '.name' >/dev/null 2>&1; then
    pass "valid JSON"
else
    fail "invalid JSON output: $VJ"
fi

NAME=$(echo "$VJ" | jq -r '.name')
[ "$NAME" = "mx" ] && pass "name=mx" || fail "name=$NAME (expected mx)"

VERSION=$(echo "$VJ" | jq -r '.version')
[ -n "$VERSION" ] && pass "version present ($VERSION)" || fail "version missing"

TARGET=$(echo "$VJ" | jq -r '.target')
[ -n "$TARGET" ] && pass "target present ($TARGET)" || fail "target missing"

PV=$(echo "$VJ" | jq -r '.plan_version')
[ "$PV" = "1" ] && pass "plan_version=1" || fail "plan_version=$PV (expected 1)"

# Check capabilities object
for cap in emit_c compile_plan m2plus ffi_c exportc diagnostics_json; do
    VAL=$(echo "$VJ" | jq -r ".capabilities.$cap")
    if [ "$VAL" = "true" ] || [ "$VAL" = "false" ]; then
        pass "capability $cap=$VAL"
    else
        fail "capability $cap missing or invalid ($VAL)"
    fi
done

# Check stdlib array
STDLIB_LEN=$(echo "$VJ" | jq '.stdlib | length')
[ "$STDLIB_LEN" -gt 0 ] && pass "stdlib array ($STDLIB_LEN modules)" || fail "stdlib array empty"

# 2. --print-targets: assert exit 0, non-empty output
echo "2. --print-targets"
TARGETS=$($MX --print-targets 2>&1)
if [ $? -eq 0 ] && [ -n "$TARGETS" ]; then
    pass "--print-targets"
else
    fail "--print-targets (exit=$?, output empty?)"
fi

# 3. Compile a trivial program
echo "3. Trivial compile"
cat > "$TMPDIR/hello.mod" <<'EOF'
MODULE hello;
FROM InOut IMPORT WriteString, WriteLn;
BEGIN
  WriteString("hello"); WriteLn
END hello.
EOF
if $MX "$TMPDIR/hello.mod" -o "$TMPDIR/hello" 2>/dev/null; then
    OUT=$("$TMPDIR/hello" 2>&1)
    [ "$OUT" = "hello" ] && pass "compile + run" || fail "output mismatch: $OUT"
else
    fail "compilation failed"
fi

# 4. --emit-c: assert .c file produced and contains main(
echo "4. --emit-c"
if $MX --emit-c "$TMPDIR/hello.mod" -o "$TMPDIR/hello_emit.c" 2>/dev/null; then
    if [ -f "$TMPDIR/hello_emit.c" ] && grep -q 'main(' "$TMPDIR/hello_emit.c"; then
        pass "--emit-c produces C with main("
    else
        fail "--emit-c output missing or no main("
    fi
else
    fail "--emit-c failed"
fi

# 5. Build plan: generate plan JSON, run compile --plan
echo "5. compile --plan"
# Copy hello.mod into a subdir so plan paths resolve relative to plan file
mkdir -p "$TMPDIR/plantest"
cp "$TMPDIR/hello.mod" "$TMPDIR/plantest/hello.mod"
cat > "$TMPDIR/plantest/plan.json" <<EOF
{
  "version": 1,
  "steps": [
    {
      "entry": "hello.mod",
      "output": "hello_plan"
    }
  ]
}
EOF
if $MX compile --plan "$TMPDIR/plantest/plan.json" 2>/dev/null; then
    if [ -f "$TMPDIR/plantest/hello_plan" ]; then
        pass "compile --plan"
    else
        fail "compile --plan produced no output"
    fi
else
    fail "compile --plan failed"
fi

# 6. plan_version round-trip: extract from --version-json, use in plan
echo "6. plan_version round-trip"
PV=$($MX --version-json 2>&1 | jq -r '.plan_version')
cat > "$TMPDIR/plantest/plan2.json" <<EOF
{
  "version": $PV,
  "steps": [
    {
      "entry": "hello.mod",
      "output": "hello_plan2"
    }
  ]
}
EOF
if $MX compile --plan "$TMPDIR/plantest/plan2.json" 2>/dev/null; then
    pass "plan_version round-trip"
else
    fail "plan_version round-trip failed"
fi

# 7. --diagnostics-json: bad input produces valid JSONL
echo "7. --diagnostics-json"
cat > "$TMPDIR/bad.mod" <<'EOF'
MODULE bad;
BEGIN
  x :=
END bad.
EOF
DIAG=$($MX --diagnostics-json "$TMPDIR/bad.mod" 2>&1 >/dev/null || true)
if [ -n "$DIAG" ]; then
    # Each line should be valid JSON with required fields
    FIRST_LINE=$(echo "$DIAG" | head -1)
    if echo "$FIRST_LINE" | jq -e '.file' >/dev/null 2>&1; then
        pass "--diagnostics-json produces valid JSONL"
    else
        fail "--diagnostics-json invalid JSON: $FIRST_LINE"
    fi
    SEV=$(echo "$FIRST_LINE" | jq -r '.severity')
    [ "$SEV" = "error" ] && pass "severity=error" || fail "severity=$SEV"
    KIND=$(echo "$FIRST_LINE" | jq -r '.kind')
    if [ "$KIND" = "parser" ] || [ "$KIND" = "semantic" ] || [ "$KIND" = "lexer" ]; then
        pass "kind=$KIND"
    else
        fail "kind=$KIND (unexpected)"
    fi
else
    fail "--diagnostics-json produced no output"
fi

# 8. --diagnostics-json: good input produces no diagnostic output
echo "8. --diagnostics-json clean output"
DIAG_GOOD=$($MX --diagnostics-json --emit-c "$TMPDIR/hello.mod" -o "$TMPDIR/hello_diag.c" 2>&1 >/dev/null || true)
if [ -z "$DIAG_GOOD" ]; then
    pass "no diagnostics for clean input"
else
    fail "unexpected diagnostics for clean input: $DIAG_GOOD"
fi

# 9. diagnostics_json capability is true
echo "9. diagnostics_json capability"
DJ_CAP=$($MX --version-json 2>&1 | jq -r '.capabilities.diagnostics_json')
[ "$DJ_CAP" = "true" ] && pass "diagnostics_json=true" || fail "diagnostics_json=$DJ_CAP"

echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="
[ "$FAIL" -eq 0 ] || exit 1
