# PROC

Predefined parameterless procedure type.

## Properties

- **Signature**: `PROCEDURE` with no parameters and no return value
- **Compatibility**: Any parameterless, non-returning procedure can be assigned to a PROC variable
- **Null value**: Can be compared to (or assigned) a procedure constant
- **Operations**: Call with `p` or `p()`

## Syntax

```modula2
VAR
  action: PROC;

PROCEDURE DoSomething;
BEGIN
  (* ... *)
END DoSomething;

PROCEDURE DoNothing;
BEGIN
END DoNothing;

action := DoSomething;
action;                   (* calls DoSomething *)
action := DoNothing;
action();                 (* also valid call syntax *)
```

## Notes

- PROC is equivalent to `PROCEDURE` as a type, i.e., `TYPE PROC = PROCEDURE`.
- A PROC variable can be reassigned at runtime, enabling simple callbacks and dispatch tables.
- Calling an uninitialized PROC variable is undefined behavior.
- For procedures with parameters or return values, define an explicit procedure type instead: `TYPE Handler = PROCEDURE(INTEGER): BOOLEAN`.
- PROC is useful for registering cleanup actions, event handlers, or initialization hooks.
