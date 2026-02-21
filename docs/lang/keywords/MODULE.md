# MODULE

Program or library module declaration. Entry point for programs. Can contain
declarations, constants, types, variables, procedures, and initialization code.

```modula2
MODULE name;
  IMPORT SomeModule;
  (* declarations *)
BEGIN
  (* initialization statements *)
END name.
```

## Example

```modula2
MODULE Hello;
FROM InOut IMPORT WriteString, WriteLn;
BEGIN
  WriteString("Hello, world!");
  WriteLn;
END Hello.
```
