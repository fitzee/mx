# FROM

Import specific symbols from a module. Imported symbols are available without
module qualification.

```modula2
FROM ModuleName IMPORT sym1, sym2, sym3;
```

## Example

```modula2
FROM InOut IMPORT WriteString, WriteLn, WriteCard;
BEGIN
  WriteString("Count: ");
  WriteCard(42, 1);
  WriteLn;
END;
```
