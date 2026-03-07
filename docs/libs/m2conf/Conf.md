# Conf

INI-style configuration file parser. Supports sections, key-value pairs, and comments. Fixed-capacity, no heap allocation.

## Why Conf?

Applications need a simple, human-editable configuration format. Conf parses the standard INI format -- `[section]` headers, `key=value` pairs, and `#` comments -- into a queryable in-memory structure. The entire parser operates on a caller-provided buffer and stores results in fixed-size internal arrays, so there is no heap allocation and no file I/O dependency (the caller loads the file).

## Format

```ini
# Comment lines start with #
name=value

[section]
key=value
another=value with spaces
```

- Lines starting with `#` are comments.
- Blank lines are ignored.
- `[name]` starts a new section.
- `key=value` pairs belong to the current section.
- Keys before any section header belong to the empty section `""`.
- Whitespace around keys and values is trimmed.

## Limits

| Resource | Maximum |
|----------|---------|
| Sections | 16 |
| Keys per section | 32 |
| Key name length | 64 chars |
| Value length | 256 chars |

## Procedures

### Parse

```modula2
PROCEDURE Parse(buf: ARRAY OF CHAR; len: CARDINAL): BOOLEAN;
```

Parse `len` bytes of INI-format text from `buf`. Returns `TRUE` on success. Calling `Parse` again replaces all previously parsed data.

### Clear

```modula2
PROCEDURE Clear;
```

Reset all parsed data, freeing all sections and keys.

### SectionCount

```modula2
PROCEDURE SectionCount(): INTEGER;
```

Return the number of sections parsed. Includes the implicit empty section if keys appeared before any `[section]` header.

### GetSectionName

```modula2
PROCEDURE GetSectionName(i: INTEGER; VAR name: ARRAY OF CHAR): BOOLEAN;
```

Copy the name of section at index `i` (0-based) into `name`. Returns `TRUE` if the index is valid.

### KeyCount

```modula2
PROCEDURE KeyCount(section: ARRAY OF CHAR): INTEGER;
```

Return the number of keys in the named section. Returns -1 if the section does not exist.

### GetKey

```modula2
PROCEDURE GetKey(section: ARRAY OF CHAR; i: INTEGER;
                 VAR key: ARRAY OF CHAR): BOOLEAN;
```

Copy the key name at index `i` within `section` into `key`. Returns `TRUE` if found.

### GetValue

```modula2
PROCEDURE GetValue(section: ARRAY OF CHAR; key: ARRAY OF CHAR;
                   VAR value: ARRAY OF CHAR): BOOLEAN;
```

Look up `key` in `section` and copy its value into `value`. Returns `TRUE` if found.

### HasKey

```modula2
PROCEDURE HasKey(section: ARRAY OF CHAR; key: ARRAY OF CHAR): BOOLEAN;
```

Returns `TRUE` if `key` exists in `section`.

## Example

```modula2
FROM Conf IMPORT Parse, GetValue, HasKey, SectionCount;
FROM InOut IMPORT WriteString, WriteLn;

VAR
  ini: ARRAY [0..1023] OF CHAR;
  val: ARRAY [0..255] OF CHAR;
BEGIN
  (* Assume ini is loaded with config text, len bytes *)
  IF Parse(ini, len) THEN
    IF GetValue("server", "port", val) THEN
      WriteString("port="); WriteString(val); WriteLn
    END;
    IF HasKey("server", "debug") THEN
      WriteString("debug enabled"); WriteLn
    END
  END
END
```
