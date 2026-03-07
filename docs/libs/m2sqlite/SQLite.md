# SQLite

## Why
Provides a prepared-statement SQLite3 interface that prevents SQL injection by design. All text columns are read into caller-provided buffers, so there is no hidden heap allocation on the Modula-2 side. Requires `-lsqlite3` at link time.

## Types

- **DB** (ADDRESS) -- Opaque database connection handle.
- **Stmt** (ADDRESS) -- Opaque prepared statement handle.
- **Status** -- `Ok`, `Error`, `Row`, `Done`, `Busy`, `Locked`.
- **ColType** -- `IntCol`, `FloatCol`, `TextCol`, `BlobCol`, `NullCol`.

## Procedures

### Connection

- `PROCEDURE Open(path: ARRAY OF CHAR; VAR db: DB): Status`
  Open or create a database file.

- `PROCEDURE Close(db: DB): Status`
  Close the database connection.

- `PROCEDURE Exec(db: DB; sql: ARRAY OF CHAR): Status`
  Execute a simple SQL statement (no result rows). Useful for DDL and pragmas.

### Prepared Statements

- `PROCEDURE Prepare(db: DB; sql: ARRAY OF CHAR; VAR stmt: Stmt): Status`
  Compile a SQL statement. Use `?` placeholders for parameters.

- `PROCEDURE Step(stmt: Stmt): Status`
  Execute one step. Returns `Row` if a result row is available, `Done` when finished.

- `PROCEDURE Reset(stmt: Stmt): Status`
  Reset a statement so it can be executed again with new bindings.

- `PROCEDURE ClearBindings(stmt: Stmt): Status`
  Clear all parameter bindings.

- `PROCEDURE Finalize(stmt: Stmt): Status`
  Destroy a prepared statement and free its resources.

### Binding Parameters

- `PROCEDURE BindInt(stmt: Stmt; idx: INTEGER; val: INTEGER): Status`
- `PROCEDURE BindLong(stmt: Stmt; idx: INTEGER; val: LONGINT): Status`
- `PROCEDURE BindReal(stmt: Stmt; idx: INTEGER; val: REAL): Status`
- `PROCEDURE BindText(stmt: Stmt; idx: INTEGER; text: ARRAY OF CHAR): Status`
- `PROCEDURE BindNull(stmt: Stmt; idx: INTEGER): Status`

Parameter indices start at 1.

### Reading Columns

- `PROCEDURE ColumnCount(stmt: Stmt): INTEGER`
- `PROCEDURE ColumnType(stmt: Stmt; idx: INTEGER): ColType`
- `PROCEDURE ColumnInt(stmt: Stmt; idx: INTEGER): INTEGER`
- `PROCEDURE ColumnLong(stmt: Stmt; idx: INTEGER): LONGINT`
- `PROCEDURE ColumnReal(stmt: Stmt; idx: INTEGER): REAL`
- `PROCEDURE ColumnText(stmt: Stmt; idx: INTEGER; VAR buf: ARRAY OF CHAR)`

Column indices start at 0.

### Other

- `PROCEDURE Changes(db: DB): INTEGER`
  Number of rows modified by the last INSERT, UPDATE, or DELETE.

- `PROCEDURE GetError(db: DB; VAR buf: ARRAY OF CHAR)`
  Copy the last error message into buf.

## Example

```modula2
MODULE SqliteDemo;

FROM InOut IMPORT WriteString, WriteLn, WriteInt;
FROM SQLite IMPORT DB, Stmt, Status, Open, Close, Exec,
                    Prepare, Step, Finalize, BindText,
                    ColumnInt, ColumnText, Ok, Row;

VAR
  db: DB;
  stmt: Stmt;
  s: Status;
  name: ARRAY [0..63] OF CHAR;

BEGIN
  s := Open("test.db", db);
  IF s # Ok THEN HALT END;

  Exec(db, "CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, name TEXT)");
  Exec(db, "INSERT INTO users (name) VALUES ('Alice')");
  Exec(db, "INSERT INTO users (name) VALUES ('Bob')");

  s := Prepare(db, "SELECT id, name FROM users", stmt);
  IF s = Ok THEN
    WHILE Step(stmt) = Row DO
      WriteInt(ColumnInt(stmt, 0), 0);
      WriteString(" ");
      ColumnText(stmt, 1, name);
      WriteString(name);
      WriteLn
    END;
    Finalize(stmt)
  END;

  Close(db)
END SqliteDemo.
```
