# CLI

Command-line argument parser with support for short and long flags, options with values, and automatic help generation. Fixed-capacity, no heap allocation.

## Why CLI?

Command-line tools need to accept flags (`-v`, `--verbose`) and options (`-o file`, `--output file`) in a consistent way. CLI provides a declarative API: register your flags and options up front, call `Parse`, then query results. It handles both `-x` and `--long` forms, consumes the next argument for options, and can print formatted usage help from the registered specs.

The module uses fixed-size internal arrays (max 16 specs), so it works without heap allocation and is suitable for use in bootstrapping tools like mxpkg.

## Types

### GetArgProc

```modula2
TYPE GetArgProc = PROCEDURE(CARDINAL, VAR ARRAY OF CHAR);
```

Callback that retrieves the argument at index `i` into the provided buffer. Typically wraps `Args.GetArg`.

## Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| Max specs | 16 | Maximum combined flags + options |
| Short name | 8 chars | Max length of short name (e.g., `-v`) |
| Long name | 32 chars | Max length of long name (e.g., `--verbose`) |

## Procedures

### AddFlag

```modula2
PROCEDURE AddFlag(short: ARRAY OF CHAR; long: ARRAY OF CHAR;
                  description: ARRAY OF CHAR);
```

Register a boolean flag. `short` is the single-character form (e.g., `"v"`), `long` is the full name (e.g., `"verbose"`). Either may be empty to disable that form. `description` is used by `PrintHelp`.

### AddOption

```modula2
PROCEDURE AddOption(short: ARRAY OF CHAR; long: ARRAY OF CHAR;
                    description: ARRAY OF CHAR);
```

Register an option that expects a value argument. When parsed, the next argument after the flag is consumed as the value. Same naming conventions as `AddFlag`.

### Parse

```modula2
PROCEDURE Parse(ac: CARDINAL; getArg: GetArgProc);
```

Parse `ac` arguments using `getArg` to retrieve each one by index. Argument 0 is typically the program name and is skipped. Flags set their presence bit; options store the following argument as their value.

### HasFlag

```modula2
PROCEDURE HasFlag(long: ARRAY OF CHAR): INTEGER;
```

Returns 1 if the flag identified by `long` was present on the command line, 0 otherwise.

### GetOption

```modula2
PROCEDURE GetOption(long: ARRAY OF CHAR; VAR buf: ARRAY OF CHAR): INTEGER;
```

If the option identified by `long` was present, copies its value into `buf` and returns 1. Otherwise returns 0 and `buf` is unchanged.

### PrintHelp

```modula2
PROCEDURE PrintHelp;
```

Print formatted usage information to stdout listing all registered flags and options with their short/long forms and descriptions.

## Example

```modula2
FROM CLI IMPORT AddFlag, AddOption, Parse, HasFlag, GetOption, PrintHelp;
FROM Args IMPORT GetArg, NumArgs;

VAR outFile: ARRAY [0..255] OF CHAR;

BEGIN
  AddFlag("v", "verbose", "Enable verbose output");
  AddOption("o", "output", "Output file path");
  Parse(NumArgs(), GetArg);

  IF HasFlag("verbose") = 1 THEN
    WriteString("verbose mode"); WriteLn
  END;

  IF GetOption("output", outFile) = 1 THEN
    WriteString("output: "); WriteString(outFile); WriteLn
  ELSE
    PrintHelp
  END
END
```
