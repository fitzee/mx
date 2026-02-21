# DEFINITION

Definition module. Declares the public interface of a module. All symbols
declared in a definition module are implicitly exported.

```modula2
DEFINITION MODULE name;
  (* type, const, var, procedure declarations *)
END name.
```

## Example

```modula2
DEFINITION MODULE Stack;
  TYPE Stack;  (* opaque type *)
  PROCEDURE Create(): Stack;
  PROCEDURE Push(s: Stack; val: INTEGER);
  PROCEDURE Pop(s: Stack): INTEGER;
END Stack.
```
