# EXPORT

Export symbols from a local module. QUALIFIED requires callers to use
Module.sym notation. In definition modules, all declared symbols are implicitly
exported.

```modula2
EXPORT sym1, sym2;
EXPORT QUALIFIED sym1, sym2;
```

## Example

```modula2
MODULE Inner;
  EXPORT QUALIFIED Count, Reset;
  VAR Count: INTEGER;
  PROCEDURE Reset; BEGIN Count := 0 END Reset;
END Inner;
(* Access as Inner.Count, Inner.Reset *)
```
