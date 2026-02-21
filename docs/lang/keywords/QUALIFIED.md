# QUALIFIED

Export qualifier used with EXPORT. Requires callers to access exported symbols
using module-qualified notation (Module.Symbol).

```modula2
EXPORT QUALIFIED sym1, sym2;
```

## Example

```modula2
MODULE Colors;
  EXPORT QUALIFIED Red, Green, Blue;
  CONST Red = 0; Green = 1; Blue = 2;
END Colors;

(* Usage: Colors.Red, Colors.Green *)
```
