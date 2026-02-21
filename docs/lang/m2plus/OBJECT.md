# OBJECT

Object type with fields and methods. Supports single inheritance
and vtable-based dynamic dispatch. Requires `--m2plus`.

## Syntax

```modula2
TYPE T = OBJECT
  field1: Type1;
  field2: Type2;
METHODS
  Method1(args): ReturnType;
  Method2(args);
END;

(* Inheritance *)
TYPE Sub = T OBJECT
  extraField: Type3;
OVERRIDES
  Method1 := SubMethod1;
END;
```

## Notes

- OBJECT declarations define both a reference type and its layout.
- Methods are dispatched through a vtable at runtime.
- Single inheritance: a subtype includes all fields and methods of
  its parent, and can override methods.
- Object values are heap-allocated; variables of object type are
  implicitly references.

## Example

```modula2
TYPE
  Shape = OBJECT
    x, y: INTEGER;
  METHODS
    Area(): REAL;
    Name(): ARRAY OF CHAR;
  END;

  Circle = Shape OBJECT
    radius: REAL;
  OVERRIDES
    Area := CircleArea;
    Name := CircleName;
  END;
```
