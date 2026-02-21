# IMPORT

Import a module. Makes the module name available as a qualifier. Access symbols
with dot notation: ModuleName.Symbol.

```modula2
IMPORT ModuleName;
```

## Example

```modula2
MODULE Demo;
IMPORT InOut;
BEGIN
  InOut.WriteString("Hello");
  InOut.WriteLn;
END Demo.
```
