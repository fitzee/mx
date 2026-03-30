# Args

Access command-line arguments passed to the program. This module is compiled into the compiler -- just import it directly with no dependency setup.

Available in PIM4 mode (the default). No special flags needed.

## Procedures

```modula2
PROCEDURE ArgCount(): CARDINAL;
```
Return the total number of command-line arguments, including the program name itself. A program run as `./myapp foo bar` has `ArgCount() = 3` (the program name plus two arguments).

```modula2
PROCEDURE GetArg(n: CARDINAL; VAR buf: ARRAY OF CHAR);
```
Copy argument number `n` into `buf`. Arguments are numbered starting from 0:

| Index | Content |
|-------|---------|
| 0 | Program name (e.g., `"./myapp"`) |
| 1 | First argument |
| 2 | Second argument |
| ... | ... |

The result is NUL-terminated. If the argument is longer than `buf` can hold, it is truncated. If `n` is out of range, `buf` receives an empty string.

## Example

```modula2
MODULE ArgsDemo;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;
FROM Args IMPORT ArgCount, GetArg;

VAR
  arg: ARRAY [0..255] OF CHAR;
  i: CARDINAL;

BEGIN
  WriteString("Program received ");
  WriteInt(INTEGER(ArgCount()), 1);
  WriteString(" argument(s):"); WriteLn;

  FOR i := 0 TO ArgCount() - 1 DO
    GetArg(i, arg);
    WriteString("  argv[");
    WriteInt(INTEGER(i), 1);
    WriteString("] = ");
    WriteString(arg); WriteLn
  END
END ArgsDemo.
```

Running `mx ArgsDemo.mod -o demo && ./demo hello world` prints:

```
Program received 3 argument(s):
  argv[0] = ./demo
  argv[1] = hello
  argv[2] = world
```
