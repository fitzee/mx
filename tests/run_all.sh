#!/bin/bash
# Run all Modula-2 test programs and report results
set -e

COMPILER="cargo run --quiet --"
PASS=0
FAIL=0
SKIP=0

# Modules that are libraries (not standalone programs)
SKIP_MODULES="MathUtils Stack ffitest exportc_test ExportCTest stream_e2e"

# FFI test: requires linking with external C file
FFI_SKIP="ffitest"
echo -n "FFI test (ffitest)... "
if $COMPILER tests/ffitest.mod tests/cadd.c -o /tmp/m2_test_ffitest 2>/tmp/m2_err_ffitest; then
    if /tmp/m2_test_ffitest > /tmp/m2_out_ffitest 2>&1; then
        expected=$(printf "3 + 4 = 7\n3 * 4 = 12\n")
        actual=$(cat /tmp/m2_out_ffitest)
        if [ "$actual" = "$expected" ]; then
            echo "PASS"
            PASS=$((PASS + 1))
        else
            echo "FAIL (output mismatch)"
            echo "  expected: $expected"
            echo "  actual:   $actual"
            FAIL=$((FAIL + 1))
        fi
    else
        echo "FAIL (runtime)"
        FAIL=$((FAIL + 1))
    fi
else
    echo "FAIL (compile)"
    cat /tmp/m2_err_ffitest
    FAIL=$((FAIL + 1))
fi

# EXPORTC test: implementation module linked with external C main
echo -n "EXPORTC test (exportc_test)... "
if $COMPILER --emit-c -I tests tests/exportc_test.mod -o /tmp/exportc_test.c 2>/tmp/m2_err_exportc; then
    if cc -c /tmp/exportc_test.c -o /tmp/exportc_test.o 2>>/tmp/m2_err_exportc && \
       cc -c tests/exportc_main.c -o /tmp/exportc_main.o 2>>/tmp/m2_err_exportc && \
       cc /tmp/exportc_test.o /tmp/exportc_main.o -o /tmp/m2_test_exportc 2>>/tmp/m2_err_exportc; then
        if /tmp/m2_test_exportc > /tmp/m2_out_exportc 2>&1; then
            expected="OK: get_value() returned 42"
            actual=$(cat /tmp/m2_out_exportc)
            if [ "$actual" = "$expected" ]; then
                echo "PASS"
                PASS=$((PASS + 1))
            else
                echo "FAIL (output mismatch)"
                echo "  expected: $expected"
                echo "  actual:   $actual"
                FAIL=$((FAIL + 1))
            fi
        else
            echo "FAIL (runtime)"
            FAIL=$((FAIL + 1))
        fi
    else
        echo "FAIL (C compile/link)"
        cat /tmp/m2_err_exportc
        FAIL=$((FAIL + 1))
    fi
else
    echo "FAIL (m2c compile)"
    cat /tmp/m2_err_exportc
    FAIL=$((FAIL + 1))
fi

for f in tests/*.mod; do
    name=$(basename "$f" .mod)

    # Skip library modules
    skip=0
    for s in $SKIP_MODULES; do
        if [ "$name" = "$s" ]; then skip=1; break; fi
    done
    if [ $skip -eq 1 ]; then
        SKIP=$((SKIP + 1))
        continue
    fi

    # Compile
    if ! $COMPILER "$f" -o "/tmp/m2_test_$name" 2>/tmp/m2_err_$name; then
        echo "FAIL (compile): $name"
        cat /tmp/m2_err_$name
        FAIL=$((FAIL + 1))
        continue
    fi

    # Run
    if ! /tmp/m2_test_$name > /tmp/m2_out_$name 2>&1; then
        echo "FAIL (runtime): $name"
        FAIL=$((FAIL + 1))
        continue
    fi

    PASS=$((PASS + 1))
done

echo ""
echo "Results: $PASS passed, $FAIL failed, $SKIP skipped"
echo "Total: $((PASS + FAIL + SKIP)) test files"

if [ $FAIL -gt 0 ]; then
    exit 1
fi
