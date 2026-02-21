# RECORD

Record type. Groups named fields of different types. Variant records use a CASE
tag to select among alternative field layouts.

```modula2
TYPE R = RECORD
  field1: Type1;
  field2: Type2;
  CASE tag: TagType OF
    val1: varField1: T1
  | val2: varField2: T2
  END;
END;
```

## Example

```modula2
TYPE Point = RECORD
  x, y: REAL;
END;

VAR p: Point;
p.x := 1.0;
p.y := 2.0;
```
