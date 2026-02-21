# BYTE

Untyped single-byte value for low-level programming. Defined in the SYSTEM module.

## Properties

- **Size**: 8 bits (1 byte)
- **Module**: Must import from `SYSTEM`
- **Compatibility**: Compatible with CHAR and any single-byte type when passed as a parameter
- **Operations**: None defined by the language

## Syntax

```modula2
FROM SYSTEM IMPORT BYTE;

PROCEDURE WriteByte(b: BYTE);
(* Accepts any byte-sized value *)

VAR
  ch: CHAR;

ch := 'X';
WriteByte(ch);   (* CHAR passed as BYTE *)
```

## Notes

- BYTE is the single-byte counterpart to WORD; it allows generic handling of byte-sized data.
- Type compatibility with BYTE applies at parameter passing boundaries.
- Useful for implementing byte-level I/O, memory buffers, and binary protocols.
- An `ARRAY OF BYTE` parameter accepts any variable regardless of type, acting as an untyped buffer (similar to `ARRAY OF WORD`).
- Like WORD, BYTE defeats the type system and should be confined to low-level modules.
