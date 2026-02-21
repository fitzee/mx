# CASE

Multi-way branch. The expression must be an ordinal type (INTEGER, CARDINAL,
CHAR, or enumeration). Cases may list multiple values separated by commas.

```modula2
CASE expr OF
  val1:        statements
| val2, val3:  statements
| val4..val5:  statements
ELSE
  statements
END;
```

## Example

```modula2
CASE ch OF
  'a'..'z': WriteString("lowercase")
| 'A'..'Z': WriteString("uppercase")
| '0'..'9': WriteString("digit")
ELSE
  WriteString("other")
END;
```
