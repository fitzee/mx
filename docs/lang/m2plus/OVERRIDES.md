# OVERRIDES

Override parent methods in a subtype OBJECT declaration. Replaces inherited method implementations with new ones. Requires `--m2plus`.

## Syntax

```modula2
TYPE Sub = Parent OBJECT
  (* additional fields *)
OVERRIDES
  MethodName := NewImplementation;
END;
```

## Notes

- OVERRIDES lists method-to-procedure bindings that replace the parent type's method implementations in the subtype's vtable.
- The replacement procedure must match the original method signature exactly.
- Methods not listed in OVERRIDES are inherited unchanged from the parent type.
- OVERRIDES appears after any METHODS section (if present) and before END.

## Example

```modula2
TYPE
  Animal = OBJECT
  METHODS
    Speak(): ARRAY OF CHAR;
  END;

  Dog = Animal OBJECT
  OVERRIDES
    Speak := DogSpeak;
  END;

PROCEDURE DogSpeak(self: Dog): ARRAY OF CHAR;
BEGIN
  RETURN "Woof";
END DogSpeak;
```
