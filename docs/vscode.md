# VS Code integration

The `tools/vscode-m2plus/` directory contains a VS Code extension that provides Modula-2/Modula-2+ language support via the `mx --lsp` language server.

## Installation

### Prerequisites

Build and install the compiler:

```bash
cd /path/to/m2
cargo build --release
sudo cp target/release/mx /usr/local/bin/mx
```

### Install the extension

```bash
cd tools/vscode-m2plus
npm install
npm run compile
ln -s "$(pwd)" ~/.vscode/extensions/m2plus
```

Restart VS Code. The extension activates when you open a `.mod` or `.def` file.

### Development mode

To run the extension without installing:

1. Open `tools/vscode-m2plus/` as a folder in VS Code.
2. Press `F5` to launch an Extension Development Host window.
3. Open a Modula-2 project in the new window.

For continuous compilation during development:

```bash
cd tools/vscode-m2plus
npm run watch
```

## Architecture

```
VS Code  <-->  vscode-languageclient  <-->  mx --lsp (stdio)
```

The extension is a thin client. All language intelligence (diagnostics, hover, completion, etc.) is provided by the mx LSP server. The extension:

1. Launches `mx --lsp` as a child process
2. Forwards LSP messages over stdio
3. Provides TextMate syntax grammar for basic highlighting
4. Registers build/run/test/clean tasks

## Settings

All settings are under the `mx.*` namespace. Configure in VS Code settings (JSON or UI).

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `mx.serverPath` | string | `"mx"` | Path to the `mx` binary. Set to full path if not on PATH. |
| `mx.m2plus` | boolean | `true` | Pass `--m2plus` flag to the LSP server |
| `mx.includePaths` | string[] | `[]` | Additional `-I` paths passed to the LSP server |
| `mx.diagnostics.debounceMs` | number | `250` | Diagnostics debounce delay in milliseconds |

The debounce setting is passed to the server via `initializationOptions`. See [LSP configuration](lsp.md#configuration) for all server-side options.

## Commands

Open the Command Palette (`Cmd+Shift+P` / `Ctrl+Shift+P`) and type "Modula-2+":

| Command | Description |
|---------|-------------|
| **Modula-2+: Restart Language Server** | Stop and restart the `mx --lsp` process |
| **Modula-2+: Reindex Workspace** | Force the server to rebuild its workspace index. Displays file and symbol counts on completion. |
| **Modula-2+: Initialize Project** | Scaffold a new project (creates `m2.toml`, `src/Main.mod`, `tests/Main.mod`) |
| **Modula-2+: Create Debug Configuration** | Create `.vscode/tasks.json` and `launch.json` for debugging. Does not overwrite existing files. |

## Tasks

The extension provides four tasks via a TaskProvider. Access them from Terminal > Run Task, or the Command Palette > "Tasks: Run Task":

| Task | Command | Description |
|------|---------|-------------|
| build | `mx build` | Compile the project (requires `m2.toml`) |
| run | `mx run` | Compile and run |
| test | `mx test` | Compile and run tests |
| clean | `mx clean` | Remove `.mx/` build directory |
| init | `mx init` | Initialize a new project |

The **build** task is assigned to the Build group; **test** to the Test group.

### Problem matcher

Tasks use the `mx` problem matcher, which parses error output in the format:

```
file:line:col: error: message
```

Errors and warnings from `mx` are highlighted in the editor and appear in the Problems panel.

## Syntax highlighting

The extension includes a TextMate grammar (`syntaxes/modula2.tmLanguage.json`) covering:

- Keywords (PIM4 + Modula-2+ extensions)
- Built-in types (`INTEGER`, `CARDINAL`, `REAL`, `BOOLEAN`, `CHAR`, etc.)
- Built-in functions (`ABS`, `CAP`, `ORD`, `CHR`, `HIGH`, `SIZE`, `NEW`, etc.)
- Comments (`(* ... *)` with nesting)
- Strings (single and double quoted)
- Numbers (decimal, hex `0AH`, octal `77B`, character codes `65C`, floats)

The LSP server provides semantic tokens for richer highlighting (procedures, types, parameters, etc.) on top of the TextMate grammar.

## Language configuration

The extension provides:

- **Bracket matching**: `()`, `[]`, `{}`, `(* *)`
- **Auto-closing pairs**: parentheses, brackets, braces, comments, strings
- **Folding**: based on block keywords (`PROCEDURE`, `MODULE`, `IF`, `BEGIN`, `RECORD`, etc.)
- **Indentation**: auto-indent after `BEGIN`, `THEN`, `ELSE`, `DO`, `OF`, etc.

## Project detection

The LSP server automatically detects projects by looking for `m2.toml` manifests. When found, include paths and `m2plus` mode are read from the manifest, and no manual `-I` configuration is needed. See [LSP configuration](lsp.md#project-detection) for details.

## Debugging

The extension provides full source-level debugging of Modula-2 programs using LLDB via the [CodeLLDB](https://marketplace.visualstudio.com/items?itemName=vadimcn.vscode-lldb) extension.

### Quick start

1. Install the **CodeLLDB** extension from the VS Code marketplace (`vadimcn.vscode-lldb`).
2. Run **Modula-2+: Create Debug Configuration** from the Command Palette.
3. Set a breakpoint by clicking the gutter next to a line number in a `.mod` file.
4. Press `F5` to build in debug mode and launch the debugger.

### What "Create Debug Configuration" does

The command creates four files in `.vscode/`:

| File | Purpose |
|------|---------|
| `tasks.json` | Build task that runs `mx build -g` |
| `launch.json` | CodeLLDB launch config with the binary name read from `m2.toml` |
| `extensions.json` | Recommends the CodeLLDB extension |
| `settings.json` | Sets `debug.allowBreakpointsEverywhere: true` (required for `.mod` breakpoints) |

Existing files are not overwritten. Delete a file and re-run the command to regenerate it.

### How it works

The `-g` flag emits `#line` directives mapping generated C back to `.mod` source lines, enabling source-level debugging. See [debug builds](toolchain.md#debug-builds) for the full compilation details.

### Debugging features

- **Breakpoints**: Click the gutter to set breakpoints on any executable line
- **Step over** (`F10`): Advance one Modula-2 statement at a time
- **Step into** (`F11`): Enter a procedure call
- **Step out** (`Shift+F11`): Return from the current procedure
- **Continue** (`F5`): Run to the next breakpoint
- **Variables**: Local variables appear in the VARIABLES panel when paused
- **Watch**: Add expressions to the WATCH panel to track values across steps
- **Call stack**: View the full call stack with Modula-2 procedure names
- **Debug console**: Type LLDB commands directly (e.g., `p myVar`, `frame variable`)

### Build artifacts in debug mode

```
src/
  Main.c          # generated C (preserved for source mapping)
  Main.o          # object file (kept for DWARF debug info)
.mx/
  bin/
    <name>        # executable with debug info
    <name>.dSYM/  # macOS debug symbol bundle
```

### Variable naming

Local variables map 1:1 to their Modula-2 names. Module-level variables appear as `Module_varName` in the debugger. Procedure names follow the same convention (`Module_ProcName` for imported procedures).

### Troubleshooting debugging

**Breakpoints not working (red dots don't appear)**:
- Ensure `debug.allowBreakpointsEverywhere` is `true` in `.vscode/settings.json`
- Re-run "Create Debug Configuration" to regenerate the settings file

**Breakpoints appear but program doesn't stop**:
- Run `mx clean && mx build -g` to force a fresh debug build
- Ensure the binary name in `launch.json` matches the `name` field in `m2.toml`
- Check the Debug Console for error messages

**Output doesn't appear when stepping**:
- This should work automatically (stdout is unbuffered in debug mode)
- If using a non-debug build, rebuild with `mx build -g`

**Debugger shows assembly instead of source**:
- The `.dSYM` bundle may be missing -- rebuild with `mx clean && mx build -g`
- Ensure the `.mod` source files haven't moved since the last build

## Output and logs

- **Output panel**: Select "Modula-2+ Language Server" from the Output panel dropdown to see LSP server stderr output (errors, warnings, debug messages).
- **Extension log**: Check the Extension Host log for client-side issues.

## Troubleshooting

### Server not starting

1. Verify `mx` works: run `mx --version-json` in a terminal.
2. If `mx` is not on PATH, set `mx.serverPath` to the full path (e.g., `/usr/local/bin/mx`).
3. Check the Output panel for error messages.

### No diagnostics

- Ensure the file extension is `.mod` or `.def`.
- Check that the file is syntactically valid enough to parse (the server reports parse errors as diagnostics).
- Try `mx.diagnostics.debounceMs: 0` for immediate feedback.

### Stale references or symbols

- Save all files, then run "Reindex Workspace" from the command palette.
- If the issue persists after reindexing, restart the language server.

### Extension not activating

- Verify the symlink exists: `ls -la ~/.vscode/extensions/m2plus`
- Ensure the extension compiled: check that `tools/vscode-m2plus/out/extension.js` exists.
- Restart VS Code.

### Build tasks not appearing

- Ensure an `m2.toml` manifest exists in the workspace root.
- Verify `mx` is accessible (the task provider uses the configured `serverPath`).
