# m2stdlib

## Why

The standard library shipped with mx. Every module listed here is compiled into the compiler itself -- you do not need `.def` files, dependency declarations in `m2.toml`, or `-I` paths to use them. Just write `FROM ModuleName IMPORT ...` and they work.

All modules in this library are available in the default PIM4 edition. You do not need `--m2plus` unless you use the M2+ concurrency modules (Thread, Mutex, Condition).

## How to use

Import procedures from any standard module directly:

```modula2
FROM InOut IMPORT WriteString, WriteLn;
FROM Strings IMPORT Assign, Length;
FROM MathLib IMPORT sqrt;
```

No `[deps]` entry in `m2.toml` is needed. The compiler recognizes these module names and provides their implementations automatically.

## Modules

### PIM4 Modules

These are the classic PIM4 (Programming in Modula-2, 4th edition) standard library modules. They work in the default compilation mode with no flags.

| Module | Purpose |
|--------|---------|
| [InOut](InOut.md) | The workhorse I/O module: print strings, integers, hex/octal to stdout; read strings and numbers from stdin; redirect I/O to files |
| [Terminal](Terminal.md) | Minimal character I/O: read/write single characters and strings. Use this when InOut is more than you need |
| [RealInOut](RealInOut.md) | Print and read floating-point numbers in general, fixed-point, and hex formats |
| [Strings](Strings.md) | String manipulation on fixed-size `ARRAY OF CHAR`: copy, concatenate, search, compare, insert, delete |
| [Storage](Storage.md) | Heap allocation (`ALLOCATE`/`DEALLOCATE`) -- the procedures behind `NEW` and `DISPOSE` |
| [MathLib](MathLib.md) | Math functions: `sqrt`, `sin`, `cos`, `exp`, `ln`, `arctan`, `entier`, plus random number generation |
| [FileSystem](FileSystem.md) | Open, close, and read/write files one character at a time |
| [BinaryIO](BinaryIO.md) | Read and write files as raw bytes, with seeking and file-size queries |
| [Args](Args.md) | Access command-line arguments: count them and retrieve each one by index |

### ISO Modules

These modules follow the ISO 10514 Modula-2 standard naming conventions. They provide the same kinds of I/O as the PIM4 modules but with a different API style (no global `Done` variable, different procedure names). They are available in PIM4 mode -- no special flags needed.

| Module | Purpose |
|--------|---------|
| [STextIO](STextIO.md) | Read/write characters, strings, lines, and whitespace-delimited tokens |
| [SWholeIO](SWholeIO.md) | Read/write integers and cardinals with field-width formatting |
| [SRealIO](SRealIO.md) | Print REAL values in scientific notation, fixed-point, or general format |
| [SLongIO](SLongIO.md) | Same as SRealIO but for LONGREAL (double-precision) values |
| SIOResult | Result codes for ISO I/O: `ReadResult` type with values `allRight`, `endOfLine`, `endOfInput` |

### Compiler-Builtin Modules

These are built into the compiler. They are always available, no import of definition files needed.

| Module | Exports | Purpose |
|--------|---------|---------|
| SYSTEM | `ADDRESS`, `ADR`, `TSIZE`, `WORD`, `BYTE` | Low-level types and operations for pointer arithmetic and memory access |
| [Args](Args.md) | `ArgCount`, `GetArg` | Command-line argument access |

### M2+ Concurrency Modules

These modules require `--m2plus` or `m2plus=true` in `m2.toml`. They wrap pthreads and provide thread-safe concurrency primitives.

| Module | Exports | Purpose |
|--------|---------|---------|
| Thread | `Fork`, `Join`, `Self`, `Sleep`, `Yield` | Create and manage threads |
| Mutex | `Init`, `Destroy`, `Lock`, `Unlock` | Mutual exclusion locks |
| Condition | `Init`, `Destroy`, `Wait`, `Signal`, `Broadcast` | Condition variables for thread coordination |

### Internal C Bridge Modules

These modules provide the C function bindings that back the standard library implementations. They are declared as `DEFINITION MODULE FOR "C"` and call libc functions directly. You would not normally import these in application code -- use the M2 modules above instead.

| Module | Wraps |
|--------|-------|
| CIO | `getchar`, `putchar`, `fopen`, `fclose`, `fread`, `fwrite`, `fseek`, `ftell`, `feof` |
| CStr | `strlen`, `memcpy`, `memmove`, `strcmp`, `strstr`, `toupper` |
| CMem | `malloc`, `free` |
| CMath | `sqrtf`, `sinf`, `cosf`, `expf`, `logf`, `atanf`, `floorf` |
| CRand | `rand`, `srand` |
| CFmt | Float formatting wrappers (backed by `m2fmt.c`) for `snprintf`/`scanf` without varargs |

## Example

A complete program that uses several standard library modules:

```modula2
MODULE StdlibDemo;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;
FROM Strings IMPORT Assign, Length, Concat;
FROM MathLib IMPORT sqrt, entier;
FROM Args IMPORT ArgCount, GetArg;

VAR
  greeting: ARRAY [0..255] OF CHAR;
  arg: ARRAY [0..255] OF CHAR;
  i: INTEGER;

BEGIN
  (* Build a string by assignment and concatenation *)
  Assign("Hello", greeting);
  Concat(greeting, " world", greeting);
  WriteString(greeting); WriteLn;
  WriteString("Length: ");
  WriteInt(INTEGER(Length(greeting)), 1); WriteLn;

  (* Math: sqrt returns REAL, use entier to truncate to INTEGER *)
  WriteString("sqrt(144) = ");
  WriteInt(entier(sqrt(144.0)), 1); WriteLn;

  (* Print all command-line arguments *)
  WriteString("Arguments (");
  WriteInt(INTEGER(ArgCount()), 1);
  WriteString("):"); WriteLn;
  FOR i := 0 TO INTEGER(ArgCount()) - 1 DO
    GetArg(CARDINAL(i), arg);
    WriteString("  [");
    WriteInt(i, 1);
    WriteString("] ");
    WriteString(arg); WriteLn
  END
END StdlibDemo.
```
