#!/bin/bash
# Run all standalone Modula-2 example programs and report results
set -e

COMPILER="cargo run --quiet --"
PASS=0
FAIL=0
SKIP=0

# Library/helper modules (not standalone programs)
SKIP_MODULES="MathUtils hashtable"

# Categories that require external libraries and can't compile standalone
SKIP_DIRS="networking async graphics compression cli"

for f in $(find examples -name '*.mod' \
    -not -path '*/demo_project/*' \
    -not -path '*/dpaint/*' \
    -not -path '*/gfx_demo_project/*' \
    -not -path '*/sock_echo_server/*' | sort); do
    name=$(basename "$f" .mod)

    # Skip library modules
    skip=0
    for s in $SKIP_MODULES; do
        if [ "$name" = "$s" ]; then skip=1; break; fi
    done

    # Skip categories requiring external libs
    for d in $SKIP_DIRS; do
        if echo "$f" | grep -q "examples/$d/"; then skip=1; break; fi
    done

    if [ $skip -eq 1 ]; then
        SKIP=$((SKIP + 1))
        continue
    fi

    dir=$(dirname "$f")
    m2plus_flag=""
    if echo "$f" | grep -q "m2plus"; then
        m2plus_flag="--m2plus"
    fi

    # Compile
    if ! $COMPILER $m2plus_flag -I "$dir" "$f" -o "/tmp/m2_test_$name" 2>/tmp/m2_err_$name; then
        echo "FAIL (compile): $f"
        cat /tmp/m2_err_$name
        FAIL=$((FAIL + 1))
        continue
    fi

    # Run
    if ! /tmp/m2_test_$name > /tmp/m2_out_$name 2>&1; then
        echo "FAIL (runtime): $f"
        FAIL=$((FAIL + 1))
        continue
    fi

    PASS=$((PASS + 1))
done

echo ""
echo "Results: $PASS passed, $FAIL failed, $SKIP skipped"
echo "Total: $((PASS + FAIL + SKIP)) files"

if [ $FAIL -gt 0 ]; then
    exit 1
fi
