MODULE HashTests;
(* Test suite for m2hash HashMap.

   Tests:
     1.  init           Init sets count=0
     2.  put_get        Put then Get returns correct value
     3.  put_update     Put same key updates value
     4.  contains       Contains TRUE for present, FALSE for absent
     5.  remove         Remove then Get fails
     6.  remove_absent  Remove absent key returns FALSE
     7.  clear          Clear resets count to 0
     8.  tombstone      Remove + Put reuses tombstone slot
     9.  hash_determ    Hash is deterministic
    10.  hash_diff      Different keys produce different hashes
    11.  fill_table     Fill table to capacity
    12.  full_reject    Put on full table returns FALSE
    13.  many_keys      Insert and verify 50 keys
    14.  empty_key      Empty string as key works
    15.  long_key       Key at max length works *)

FROM SYSTEM IMPORT ADDRESS, ADR;
FROM InOut IMPORT WriteString, WriteLn, WriteInt, WriteCard;
FROM HashMap IMPORT Bucket, Map, Init, Clear, Put, Get,
                    Contains, Remove, Count, Hash;

VAR
  passed, failed, total: INTEGER;

PROCEDURE Check(name: ARRAY OF CHAR; cond: BOOLEAN);
BEGIN
  INC(total);
  IF cond THEN
    INC(passed)
  ELSE
    INC(failed);
    WriteString("FAIL: "); WriteString(name); WriteLn
  END
END Check;

(* ── Test 1: Init ──────────────────────────────────── *)

PROCEDURE TestInit;
VAR m: Map; buckets: ARRAY [0..15] OF Bucket;
BEGIN
  Init(m, ADR(buckets), 16);
  Check("init: count=0", Count(m) = 0)
END TestInit;

(* ── Test 2: Put + Get ─────────────────────────────── *)

PROCEDURE TestPutGet;
VAR m: Map; buckets: ARRAY [0..31] OF Bucket;
    v: INTEGER; ok: BOOLEAN;
BEGIN
  Init(m, ADR(buckets), 32);
  ok := Put(m, "hello", 42);
  Check("put_get: put ok", ok);
  Check("put_get: count=1", Count(m) = 1);
  ok := Get(m, "hello", v);
  Check("put_get: get ok", ok);
  Check("put_get: val=42", v = 42);
  (* Second key *)
  ok := Put(m, "world", 99);
  Check("put_get: put2 ok", ok);
  Check("put_get: count=2", Count(m) = 2);
  ok := Get(m, "world", v);
  Check("put_get: get2 ok", ok);
  Check("put_get: val=99", v = 99);
  (* First key still there *)
  ok := Get(m, "hello", v);
  Check("put_get: get1 still ok", ok);
  Check("put_get: val1=42", v = 42)
END TestPutGet;

(* ── Test 3: Put update ────────────────────────────── *)

PROCEDURE TestPutUpdate;
VAR m: Map; buckets: ARRAY [0..15] OF Bucket;
    v: INTEGER; ok: BOOLEAN;
BEGIN
  Init(m, ADR(buckets), 16);
  ok := Put(m, "key", 10);
  ok := Put(m, "key", 20);
  Check("update: count=1", Count(m) = 1);
  ok := Get(m, "key", v);
  Check("update: val=20", v = 20)
END TestPutUpdate;

(* ── Test 4: Contains ──────────────────────────────── *)

PROCEDURE TestContains;
VAR m: Map; buckets: ARRAY [0..15] OF Bucket;
    ok: BOOLEAN;
BEGIN
  Init(m, ADR(buckets), 16);
  ok := Put(m, "abc", 1);
  Check("contains: present", Contains(m, "abc"));
  Check("contains: absent", NOT Contains(m, "xyz"))
END TestContains;

(* ── Test 5: Remove ────────────────────────────────── *)

PROCEDURE TestRemove;
VAR m: Map; buckets: ARRAY [0..15] OF Bucket;
    v: INTEGER; ok: BOOLEAN;
BEGIN
  Init(m, ADR(buckets), 16);
  ok := Put(m, "del", 77);
  Check("remove: count=1", Count(m) = 1);
  ok := Remove(m, "del");
  Check("remove: ok", ok);
  Check("remove: count=0", Count(m) = 0);
  ok := Get(m, "del", v);
  Check("remove: get fails", NOT ok)
END TestRemove;

(* ── Test 6: Remove absent ─────────────────────────── *)

PROCEDURE TestRemoveAbsent;
VAR m: Map; buckets: ARRAY [0..15] OF Bucket;
    ok: BOOLEAN;
BEGIN
  Init(m, ADR(buckets), 16);
  ok := Remove(m, "nope");
  Check("remove_absent: FALSE", NOT ok)
END TestRemoveAbsent;

(* ── Test 7: Clear ─────────────────────────────────── *)

PROCEDURE TestClear;
VAR m: Map; buckets: ARRAY [0..31] OF Bucket;
    ok: BOOLEAN;
BEGIN
  Init(m, ADR(buckets), 32);
  ok := Put(m, "a", 1);
  ok := Put(m, "b", 2);
  ok := Put(m, "c", 3);
  Check("clear: count=3", Count(m) = 3);
  Clear(m);
  Check("clear: count=0", Count(m) = 0);
  Check("clear: a gone", NOT Contains(m, "a"))
END TestClear;

(* ── Test 8: Tombstone reuse ───────────────────────── *)

PROCEDURE TestTombstone;
VAR m: Map; buckets: ARRAY [0..15] OF Bucket;
    v: INTEGER; ok: BOOLEAN;
BEGIN
  Init(m, ADR(buckets), 16);
  ok := Put(m, "alpha", 1);
  ok := Put(m, "beta", 2);
  ok := Remove(m, "alpha");
  Check("tomb: count=1", Count(m) = 1);
  (* Re-insert same key should reuse tombstone *)
  ok := Put(m, "alpha", 10);
  Check("tomb: put ok", ok);
  Check("tomb: count=2", Count(m) = 2);
  ok := Get(m, "alpha", v);
  Check("tomb: val=10", v = 10);
  (* beta still accessible *)
  ok := Get(m, "beta", v);
  Check("tomb: beta=2", v = 2)
END TestTombstone;

(* ── Test 9: Hash deterministic ────────────────────── *)

PROCEDURE TestHashDeterministic;
VAR h1, h2: CARDINAL;
BEGIN
  h1 := Hash("test_string");
  h2 := Hash("test_string");
  Check("hash_determ: same", h1 = h2)
END TestHashDeterministic;

(* ── Test 10: Hash different ───────────────────────── *)

PROCEDURE TestHashDifferent;
VAR h1, h2: CARDINAL;
BEGIN
  h1 := Hash("foo");
  h2 := Hash("bar");
  Check("hash_diff: foo#bar", h1 # h2)
END TestHashDifferent;

(* ── Test 11: Fill table ───────────────────────────── *)

PROCEDURE TestFillTable;
VAR m: Map; buckets: ARRAY [0..3] OF Bucket;
    ok: BOOLEAN; v: INTEGER;
BEGIN
  Init(m, ADR(buckets), 4);
  ok := Put(m, "a", 1); Check("fill: a ok", ok);
  ok := Put(m, "b", 2); Check("fill: b ok", ok);
  ok := Put(m, "c", 3); Check("fill: c ok", ok);
  ok := Put(m, "d", 4); Check("fill: d ok", ok);
  Check("fill: count=4", Count(m) = 4);
  ok := Get(m, "a", v); Check("fill: get a", ok AND (v = 1));
  ok := Get(m, "b", v); Check("fill: get b", ok AND (v = 2));
  ok := Get(m, "c", v); Check("fill: get c", ok AND (v = 3));
  ok := Get(m, "d", v); Check("fill: get d", ok AND (v = 4))
END TestFillTable;

(* ── Test 12: Full reject ──────────────────────────── *)

PROCEDURE TestFullReject;
VAR m: Map; buckets: ARRAY [0..3] OF Bucket;
    ok: BOOLEAN;
BEGIN
  Init(m, ADR(buckets), 4);
  ok := Put(m, "a", 1);
  ok := Put(m, "b", 2);
  ok := Put(m, "c", 3);
  ok := Put(m, "d", 4);
  ok := Put(m, "e", 5);
  Check("full_reject: FALSE", NOT ok);
  Check("full_reject: count=4", Count(m) = 4)
END TestFullReject;

(* ── Test 13: Many keys ───────────────────────────── *)

PROCEDURE TestManyKeys;
VAR m: Map; buckets: ARRAY [0..127] OF Bucket;
    ok: BOOLEAN; v: INTEGER;
    i: CARDINAL;
    key: ARRAY [0..7] OF CHAR;
BEGIN
  Init(m, ADR(buckets), 128);
  i := 0;
  WHILE i < 50 DO
    key[0] := 'k';
    key[1] := CHR(ORD('0') + (i DIV 10));
    key[2] := CHR(ORD('0') + (i MOD 10));
    key[3] := 0C;
    ok := Put(m, key, INTEGER(i));
    Check("many: put ok", ok);
    INC(i)
  END;
  Check("many: count=50", Count(m) = 50);
  (* Verify a few *)
  ok := Get(m, "k00", v); Check("many: k00=0", ok AND (v = 0));
  ok := Get(m, "k25", v); Check("many: k25=25", ok AND (v = 25));
  ok := Get(m, "k49", v); Check("many: k49=49", ok AND (v = 49))
END TestManyKeys;

(* ── Test 14: Empty key ───────────────────────────── *)

PROCEDURE TestEmptyKey;
VAR m: Map; buckets: ARRAY [0..15] OF Bucket;
    v: INTEGER; ok: BOOLEAN;
BEGIN
  Init(m, ADR(buckets), 16);
  ok := Put(m, "", 55);
  Check("empty_key: put ok", ok);
  ok := Get(m, "", v);
  Check("empty_key: get ok", ok);
  Check("empty_key: val=55", v = 55)
END TestEmptyKey;

(* ── Test 15: Long key ─────────────────────────────── *)

PROCEDURE TestLongKey;
VAR m: Map; buckets: ARRAY [0..15] OF Bucket;
    longk: ARRAY [0..63] OF CHAR;
    v: INTEGER; ok: BOOLEAN;
    i: CARDINAL;
BEGIN
  Init(m, ADR(buckets), 16);
  (* Fill key with 63 'x' chars *)
  i := 0;
  WHILE i < 63 DO
    longk[i] := 'x';
    INC(i)
  END;
  longk[63] := 0C;
  ok := Put(m, longk, 999);
  Check("long_key: put ok", ok);
  ok := Get(m, longk, v);
  Check("long_key: get ok", ok);
  Check("long_key: val=999", v = 999)
END TestLongKey;

BEGIN
  passed := 0;
  failed := 0;
  total := 0;

  TestInit;
  TestPutGet;
  TestPutUpdate;
  TestContains;
  TestRemove;
  TestRemoveAbsent;
  TestClear;
  TestTombstone;
  TestHashDeterministic;
  TestHashDifferent;
  TestFillTable;
  TestFullReject;
  TestManyKeys;
  TestEmptyKey;
  TestLongKey;

  WriteLn;
  WriteString("m2hash: ");
  WriteInt(passed, 0); WriteString(" passed, ");
  WriteInt(failed, 0); WriteString(" failed, ");
  WriteInt(total, 0); WriteString(" total"); WriteLn;
  IF failed = 0 THEN
    WriteString("ALL TESTS PASSED"); WriteLn
  END
END HashTests.
