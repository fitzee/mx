MODULE SQLiteTests;
(* Deterministic test suite for m2sqlite.

   Tests:
     1. Open/close in-memory database
     2. CREATE TABLE via Exec
     3. INSERT via Exec
     4. SELECT with prepared statement
     5. Prepared statement with BindInt
     6. Prepared statement with BindText
     7. ColumnCount check
     8. ColumnType check
     9. Multiple rows iteration
    10. Reset and re-step
    11. Error handling — bad SQL
    12. GetError message
    13. BindNull and NullCol detection *)

FROM InOut IMPORT WriteString, WriteLn, WriteInt;
FROM SQLite IMPORT DB, Stmt, Status, ColType,
                   Open, Close, Exec,
                   Prepare, Finalize, Step, Reset,
                   BindInt, BindReal, BindText, BindNull,
                   ColumnCount, ColumnType,
                   ColumnInt, ColumnReal, ColumnText,
                   GetError;

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

(* ── Test 1: Open/close ────────────────────────────── *)

PROCEDURE TestOpenClose;
VAR db: DB; s: Status;
BEGIN
  s := Open(":memory:", db);
  Check("open: status ok", s = Ok);
  s := Close(db);
  Check("close: status ok", s = Ok)
END TestOpenClose;

(* ── Test 2: CREATE TABLE ──────────────────────────── *)

VAR gdb: DB;  (* shared database for remaining tests *)

PROCEDURE TestCreateTable;
VAR s: Status;
BEGIN
  s := Open(":memory:", gdb);
  Check("create: open ok", s = Ok);
  s := Exec(gdb, "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, age INTEGER)");
  Check("create: table ok", s = Ok)
END TestCreateTable;

(* ── Test 3: INSERT via Exec ───────────────────────── *)

PROCEDURE TestInsertExec;
VAR s: Status;
BEGIN
  s := Exec(gdb, "INSERT INTO users (id, name, age) VALUES (1, 'Alice', 30)");
  Check("insert: row 1 ok", s = Ok);
  s := Exec(gdb, "INSERT INTO users (id, name, age) VALUES (2, 'Bob', 25)");
  Check("insert: row 2 ok", s = Ok);
  s := Exec(gdb, "INSERT INTO users (id, name, age) VALUES (3, 'Carol', 35)");
  Check("insert: row 3 ok", s = Ok)
END TestInsertExec;

(* ── Test 4: SELECT with prepared statement ────────── *)

PROCEDURE TestSelectPrepared;
VAR
  stmt: Stmt;
  s: Status;
  id, age: INTEGER;
  name: ARRAY [0..63] OF CHAR;
BEGIN
  s := Prepare(gdb, "SELECT id, name, age FROM users WHERE id = 1", stmt);
  Check("select: prepare ok", s = Ok);

  s := Step(stmt);
  Check("select: step row", s = Row);

  id := ColumnInt(stmt, 0);
  Check("select: id=1", id = 1);

  ColumnText(stmt, 1, name);
  Check("select: name=Alice", name[0] = 'A');
  Check("select: name[1]=l", name[1] = 'l');

  age := ColumnInt(stmt, 2);
  Check("select: age=30", age = 30);

  s := Step(stmt);
  Check("select: step done", s = Done);

  s := Finalize(stmt);
  Check("select: finalize ok", s = Ok)
END TestSelectPrepared;

(* ── Test 5: BindInt ───────────────────────────────── *)

PROCEDURE TestBindInt;
VAR
  stmt: Stmt;
  s: Status;
  age: INTEGER;
  name: ARRAY [0..63] OF CHAR;
BEGIN
  s := Prepare(gdb, "SELECT name, age FROM users WHERE age > ?", stmt);
  Check("bindint: prepare ok", s = Ok);

  s := BindInt(stmt, 1, 28);
  Check("bindint: bind ok", s = Ok);

  (* should get Alice (30) and Carol (35), not Bob (25) *)
  s := Step(stmt);
  Check("bindint: step row1", s = Row);
  age := ColumnInt(stmt, 1);
  Check("bindint: age1>=29", age >= 29);

  s := Step(stmt);
  Check("bindint: step row2", s = Row);
  age := ColumnInt(stmt, 1);
  Check("bindint: age2>=29", age >= 29);

  s := Step(stmt);
  Check("bindint: step done", s = Done);

  s := Finalize(stmt);
  Check("bindint: finalize ok", s = Ok)
END TestBindInt;

(* ── Test 6: BindText ──────────────────────────────── *)

PROCEDURE TestBindText;
VAR
  stmt: Stmt;
  s: Status;
  id: INTEGER;
BEGIN
  s := Prepare(gdb, "SELECT id FROM users WHERE name = ?", stmt);
  Check("bindtext: prepare ok", s = Ok);

  s := BindText(stmt, 1, "Bob");
  Check("bindtext: bind ok", s = Ok);

  s := Step(stmt);
  Check("bindtext: step row", s = Row);

  id := ColumnInt(stmt, 0);
  Check("bindtext: id=2", id = 2);

  s := Step(stmt);
  Check("bindtext: step done", s = Done);

  s := Finalize(stmt);
  Check("bindtext: finalize ok", s = Ok)
END TestBindText;

(* ── Test 7: ColumnCount ───────────────────────────── *)

PROCEDURE TestColumnCount;
VAR
  stmt: Stmt;
  s: Status;
  n: INTEGER;
BEGIN
  s := Prepare(gdb, "SELECT id, name, age FROM users LIMIT 1", stmt);
  Check("colcount: prepare ok", s = Ok);

  n := ColumnCount(stmt);
  Check("colcount: n=3", n = 3);

  s := Finalize(stmt);
  Check("colcount: finalize ok", s = Ok)
END TestColumnCount;

(* ── Test 8: ColumnType ────────────────────────────── *)

PROCEDURE TestColumnType;
VAR
  stmt: Stmt;
  s: Status;
  ct: ColType;
BEGIN
  s := Prepare(gdb, "SELECT id, name, age FROM users WHERE id = 1", stmt);
  Check("coltype: prepare ok", s = Ok);

  s := Step(stmt);
  Check("coltype: step row", s = Row);

  ct := ColumnType(stmt, 0);
  Check("coltype: id is int", ct = IntCol);

  ct := ColumnType(stmt, 1);
  Check("coltype: name is text", ct = TextCol);

  ct := ColumnType(stmt, 2);
  Check("coltype: age is int", ct = IntCol);

  s := Finalize(stmt);
  Check("coltype: finalize ok", s = Ok)
END TestColumnType;

(* ── Test 9: Multiple rows iteration ───────────────── *)

PROCEDURE TestMultipleRows;
VAR
  stmt: Stmt;
  s: Status;
  count: INTEGER;
BEGIN
  s := Prepare(gdb, "SELECT id FROM users ORDER BY id", stmt);
  Check("multi: prepare ok", s = Ok);

  count := 0;
  s := Step(stmt);
  WHILE s = Row DO
    INC(count);
    s := Step(stmt)
  END;
  Check("multi: count=3", count = 3);
  Check("multi: done", s = Done);

  s := Finalize(stmt);
  Check("multi: finalize ok", s = Ok)
END TestMultipleRows;

(* ── Test 10: Reset and re-step ────────────────────── *)

PROCEDURE TestReset;
VAR
  stmt: Stmt;
  s: Status;
  id1, id2: INTEGER;
BEGIN
  s := Prepare(gdb, "SELECT id FROM users WHERE id = 1", stmt);
  Check("reset: prepare ok", s = Ok);

  s := Step(stmt);
  Check("reset: step1 row", s = Row);
  id1 := ColumnInt(stmt, 0);
  Check("reset: id1=1", id1 = 1);

  s := Reset(stmt);
  Check("reset: reset ok", s = Ok);

  s := Step(stmt);
  Check("reset: step2 row", s = Row);
  id2 := ColumnInt(stmt, 0);
  Check("reset: id2=1", id2 = 1);

  s := Finalize(stmt);
  Check("reset: finalize ok", s = Ok)
END TestReset;

(* ── Test 11: Error handling — bad SQL ─────────────── *)

PROCEDURE TestBadSQL;
VAR s: Status;
BEGIN
  s := Exec(gdb, "THIS IS NOT VALID SQL");
  Check("badsql: error status", s = Error)
END TestBadSQL;

(* ── Test 12: GetError message ─────────────────────── *)

PROCEDURE TestGetError;
VAR
  s: Status;
  stmt: Stmt;
  errbuf: ARRAY [0..255] OF CHAR;
BEGIN
  (* trigger an error so sqlite3_errmsg has something *)
  s := Prepare(gdb, "SELECT * FROM nonexistent_table", stmt);
  Check("geterr: prepare fails", s = Error);

  GetError(gdb, errbuf);
  (* errmsg should contain something non-empty *)
  Check("geterr: msg not empty", errbuf[0] # 0C)
END TestGetError;

(* ── Test 13: BindNull and NullCol ─────────────────── *)

PROCEDURE TestBindNull;
VAR
  stmt: Stmt;
  s: Status;
  ct: ColType;
BEGIN
  s := Exec(gdb, "CREATE TABLE nulltest (id INTEGER, val TEXT)");
  Check("bindnull: create ok", s = Ok);

  s := Prepare(gdb, "INSERT INTO nulltest (id, val) VALUES (?, ?)", stmt);
  Check("bindnull: prepare ok", s = Ok);

  s := BindInt(stmt, 1, 1);
  Check("bindnull: bind id ok", s = Ok);

  s := BindNull(stmt, 2);
  Check("bindnull: bind null ok", s = Ok);

  s := Step(stmt);
  Check("bindnull: insert done", s = Done);

  s := Finalize(stmt);
  Check("bindnull: finalize ok", s = Ok);

  (* read back and check null column *)
  s := Prepare(gdb, "SELECT val FROM nulltest WHERE id = 1", stmt);
  Check("bindnull: sel prepare ok", s = Ok);

  s := Step(stmt);
  Check("bindnull: step row", s = Row);

  ct := ColumnType(stmt, 0);
  Check("bindnull: type is null", ct = NullCol);

  s := Finalize(stmt);
  Check("bindnull: sel finalize ok", s = Ok)
END TestBindNull;

BEGIN
  passed := 0;
  failed := 0;
  total := 0;

  WriteString("m2sqlite test suite"); WriteLn;
  WriteString("==================="); WriteLn;

  TestOpenClose;
  TestCreateTable;
  TestInsertExec;
  TestSelectPrepared;
  TestBindInt;
  TestBindText;
  TestColumnCount;
  TestColumnType;
  TestMultipleRows;
  TestReset;
  TestBadSQL;
  TestGetError;
  TestBindNull;

  WriteLn;
  WriteString("m2sqlite: ");
  WriteInt(passed, 0); WriteString(" passed, ");
  WriteInt(failed, 0); WriteString(" failed, ");
  WriteInt(total, 0); WriteString(" total"); WriteLn;

  IF failed > 0 THEN
    WriteString("*** FAILURES ***"); WriteLn
  ELSE
    WriteString("*** ALL TESTS PASSED ***"); WriteLn
  END;

  Close(gdb)
END SQLiteTests.
