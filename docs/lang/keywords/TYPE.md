# TYPE

Type declaration. Creates a named type from a type expression. In definition
modules, opaque types are declared without a body.

```modula2
TYPE
  name = typeExpression;
  opaqueName;              (* in .def files only *)
```

## Example

```modula2
TYPE
  String = ARRAY [0..255] OF CHAR;
  Color = (Red, Green, Blue);
  NodePtr = POINTER TO Node;
  Node = RECORD
    value: INTEGER;
    next: NodePtr;
  END;
```
