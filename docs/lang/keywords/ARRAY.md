# ARRAY

Array type. Fixed-size arrays have explicit index bounds. Open arrays (ARRAY OF T)
are allowed only as formal parameters. Multi-dimensional arrays use comma-separated
index ranges.

```modula2
ARRAY [lo..hi] OF ElementType
ARRAY OF T                         (* open array parameter *)
ARRAY [a..b],[c..d] OF T           (* multi-dimensional *)
```

## Example

```modula2
TYPE
  Vector = ARRAY [0..2] OF REAL;
  Matrix = ARRAY [0..3],[0..3] OF REAL;

PROCEDURE Sum(a: ARRAY OF INTEGER): INTEGER;
```
