# IN

Set membership test. Returns TRUE if the element is a member of the set. The
element must be compatible with the set's base type.

```modula2
elem IN setExpr
```

## Example

```modula2
TYPE Days = SET OF [Mon..Sun];
VAR weekend: Days;

weekend := Days{Sat, Sun};
IF today IN weekend THEN
  WriteString("day off");
END;
```
