# METHODS

Methods section in an OBJECT type declaration. Lists the procedure signatures that instances of the object type support. Requires `--m2plus`.

## Syntax

```modula2
TYPE T = OBJECT
  (* fields *)
METHODS
  ProcName(params): ReturnType;
  AnotherProc(params);
END;
```

## Notes

- Each method signature declares a procedure that can be called on instances of the object type.
- Methods receive the object instance as an implicit first argument.
- Method calls use dot notation: `obj.Method(args)`.
- Methods are dispatched through the vtable, enabling polymorphism when the object is accessed through a parent type.
- Method implementations are separate procedure declarations that are bound to the object type.

## Example

```modula2
TYPE
  Counter = OBJECT
    count: INTEGER;
  METHODS
    Increment();
    Reset();
    Value(): INTEGER;
  END;

(* Usage *)
VAR c: Counter;
BEGIN
  NEW(c);
  c.count := 0;
  c.Increment();
  WriteInt(c.Value(), 0); WriteLn;
END MethodsDemo.
```
