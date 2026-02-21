# IMPLEMENTATION

Implementation module. Provides the implementation for a corresponding
definition module. Reveals opaque types and defines procedure bodies.

```modula2
IMPLEMENTATION MODULE name;
  (* full declarations and procedure bodies *)
BEGIN
  (* optional initialization *)
END name.
```

## Example

```modula2
IMPLEMENTATION MODULE Stack;
TYPE Stack = POINTER TO RECORD
  data: ARRAY [0..99] OF INTEGER;
  top: INTEGER;
END;
PROCEDURE Create(): Stack;
VAR s: Stack;
BEGIN NEW(s); s^.top := 0; RETURN s END Create;
END Stack.
```
