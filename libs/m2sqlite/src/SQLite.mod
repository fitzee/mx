IMPLEMENTATION MODULE SQLite;

FROM SYSTEM IMPORT ADDRESS, ADR;
FROM SQLiteBridge IMPORT m2_sqlite_open, m2_sqlite_close, m2_sqlite_exec,
                         m2_sqlite_prepare, m2_sqlite_finalize,
                         m2_sqlite_step, m2_sqlite_reset,
                         m2_sqlite_clear_bindings, m2_sqlite_changes,
                         m2_sqlite_bind_int, m2_sqlite_bind_int64,
                         m2_sqlite_bind_real,
                         m2_sqlite_bind_text, m2_sqlite_bind_null,
                         m2_sqlite_column_count, m2_sqlite_column_type,
                         m2_sqlite_column_int, m2_sqlite_column_int64,
                         m2_sqlite_column_real,
                         m2_sqlite_column_text, m2_sqlite_errmsg;

(* ── Internal helpers ──────────────────────────────── *)

PROCEDURE MapStatus(rc: INTEGER): Status;
BEGIN
  IF rc = 0 THEN RETURN Ok
  ELSIF rc = 1 THEN RETURN Row
  ELSIF rc = 2 THEN RETURN Done
  ELSIF rc = 3 THEN RETURN Busy
  ELSIF rc = 4 THEN RETURN Locked
  ELSE RETURN Error
  END
END MapStatus;

PROCEDURE MapBindStatus(rc: INTEGER): Status;
BEGIN
  IF rc = 0 THEN RETURN Ok ELSE RETURN Error END
END MapBindStatus;

PROCEDURE MapColType(t: INTEGER): ColType;
BEGIN
  IF t = 0 THEN RETURN IntCol
  ELSIF t = 1 THEN RETURN FloatCol
  ELSIF t = 2 THEN RETURN TextCol
  ELSIF t = 3 THEN RETURN BlobCol
  ELSE RETURN NullCol
  END
END MapColType;

(* ── Connection ────────────────────────────────────── *)

PROCEDURE Open(path: ARRAY OF CHAR; VAR db: DB): Status;
VAR rc: INTEGER;
BEGIN
  rc := m2_sqlite_open(ADR(path), db);
  RETURN MapBindStatus(rc)
END Open;

PROCEDURE Close(db: DB): Status;
VAR rc: INTEGER;
BEGIN
  rc := m2_sqlite_close(db);
  RETURN MapBindStatus(rc)
END Close;

PROCEDURE Exec(db: DB; sql: ARRAY OF CHAR): Status;
VAR rc: INTEGER;
BEGIN
  rc := m2_sqlite_exec(db, ADR(sql));
  RETURN MapBindStatus(rc)
END Exec;

(* ── Prepared statements ───────────────────────────── *)

PROCEDURE Prepare(db: DB; sql: ARRAY OF CHAR; VAR stmt: Stmt): Status;
VAR rc: INTEGER;
BEGIN
  rc := m2_sqlite_prepare(db, ADR(sql), stmt);
  RETURN MapBindStatus(rc)
END Prepare;

PROCEDURE Finalize(stmt: Stmt): Status;
VAR rc: INTEGER;
BEGIN
  rc := m2_sqlite_finalize(stmt);
  RETURN MapBindStatus(rc)
END Finalize;

PROCEDURE Step(stmt: Stmt): Status;
VAR rc: INTEGER;
BEGIN
  rc := m2_sqlite_step(stmt);
  RETURN MapStatus(rc)
END Step;

PROCEDURE Reset(stmt: Stmt): Status;
VAR rc: INTEGER;
BEGIN
  rc := m2_sqlite_reset(stmt);
  RETURN MapBindStatus(rc)
END Reset;

PROCEDURE ClearBindings(stmt: Stmt): Status;
VAR rc: INTEGER;
BEGIN
  rc := m2_sqlite_clear_bindings(stmt);
  RETURN MapBindStatus(rc)
END ClearBindings;

PROCEDURE Changes(db: DB): INTEGER;
BEGIN
  RETURN m2_sqlite_changes(db)
END Changes;

(* ── Binding ───────────────────────────────────────── *)

PROCEDURE BindInt(stmt: Stmt; idx: INTEGER; val: INTEGER): Status;
VAR rc: INTEGER;
BEGIN
  rc := m2_sqlite_bind_int(stmt, idx, val);
  RETURN MapBindStatus(rc)
END BindInt;

PROCEDURE BindLong(stmt: Stmt; idx: INTEGER; val: LONGINT): Status;
VAR rc: INTEGER;
BEGIN
  rc := m2_sqlite_bind_int64(stmt, idx, val);
  RETURN MapBindStatus(rc)
END BindLong;

PROCEDURE BindReal(stmt: Stmt; idx: INTEGER; val: REAL): Status;
VAR rc: INTEGER;
BEGIN
  rc := m2_sqlite_bind_real(stmt, idx, val);
  RETURN MapBindStatus(rc)
END BindReal;

PROCEDURE BindText(stmt: Stmt; idx: INTEGER; text: ARRAY OF CHAR): Status;
VAR rc: INTEGER;
BEGIN
  rc := m2_sqlite_bind_text(stmt, idx, ADR(text));
  RETURN MapBindStatus(rc)
END BindText;

PROCEDURE BindNull(stmt: Stmt; idx: INTEGER): Status;
VAR rc: INTEGER;
BEGIN
  rc := m2_sqlite_bind_null(stmt, idx);
  RETURN MapBindStatus(rc)
END BindNull;

(* ── Column access ─────────────────────────────────── *)

PROCEDURE ColumnCount(stmt: Stmt): INTEGER;
BEGIN
  RETURN m2_sqlite_column_count(stmt)
END ColumnCount;

PROCEDURE ColumnType(stmt: Stmt; idx: INTEGER): ColType;
BEGIN
  RETURN MapColType(m2_sqlite_column_type(stmt, idx))
END ColumnType;

PROCEDURE ColumnInt(stmt: Stmt; idx: INTEGER): INTEGER;
BEGIN
  RETURN m2_sqlite_column_int(stmt, idx)
END ColumnInt;

PROCEDURE ColumnLong(stmt: Stmt; idx: INTEGER): LONGINT;
BEGIN
  RETURN m2_sqlite_column_int64(stmt, idx)
END ColumnLong;

PROCEDURE ColumnReal(stmt: Stmt; idx: INTEGER): REAL;
BEGIN
  RETURN m2_sqlite_column_real(stmt, idx)
END ColumnReal;

PROCEDURE ColumnText(stmt: Stmt; idx: INTEGER; VAR buf: ARRAY OF CHAR);
BEGIN
  m2_sqlite_column_text(stmt, idx, ADR(buf), HIGH(buf) + 1)
END ColumnText;

(* ── Error reporting ───────────────────────────────── *)

PROCEDURE GetError(db: DB; VAR buf: ARRAY OF CHAR);
BEGIN
  m2_sqlite_errmsg(db, ADR(buf), HIGH(buf) + 1)
END GetError;

END SQLite.
