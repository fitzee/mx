# EXIT

Exit from a LOOP statement. Transfers control to the statement immediately after
the enclosing LOOP..END. Only valid inside a LOOP.

```modula2
LOOP
  (* ... *)
  IF condition THEN EXIT END;
  (* ... *)
END;
(* control continues here after EXIT *)
```

## Example

```modula2
LOOP
  ch := GetChar();
  IF ch = 0C THEN EXIT END;
  Append(buf, ch);
END;
```
