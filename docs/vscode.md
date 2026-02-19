# VS Code integration

The `tools/vscode-m2plus/` directory contains a VS Code extension that provides Modula-2/Modula-2+ language support via the `m2c --lsp` language server.

## Installation

### Prerequisites

Build and install the compiler:

```bash
cd /path/to/m2
cargo build --release
sudo cp target/release/m2c /usr/local/bin/m2c
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
VS Code  <-->  vscode-languageclient  <-->  m2c --lsp (stdio)
```

The extension is a thin client. All language intelligence (diagnostics, hover, completion, etc.) is provided by the m2c LSP server. The extension:

1. Launches `m2c --lsp` as a child process
2. Forwards LSP messages over stdio
3. Provides TextMate syntax grammar for basic highlighting
4. Registers build/run/test/clean tasks

## Settings

All settings are under the `m2c.*` namespace. Configure in VS Code settings (JSON or UI).

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `m2c.serverPath` | string | `"m2c"` | Path to the `m2c` binary. Set to full path if not on PATH. |
| `m2c.m2plus` | boolean | `true` | Pass `--m2plus` flag to the LSP server |
| `m2c.includePaths` | string[] | `[]` | Additional `-I` paths passed to the LSP server |
| `m2c.diagnostics.debounceMs` | number | `250` | Diagnostics debounce delay in milliseconds |

The debounce setting is passed to the server via `initializationOptions`. See [LSP configuration](lsp.md#configuration) for all server-side options.

## Commands

Open the Command Palette (`Cmd+Shift+P` / `Ctrl+Shift+P`) and type "Modula-2+":

| Command | Description |
|---------|-------------|
| **Modula-2+: Restart Language Server** | Stop and restart the `m2c --lsp` process |
| **Modula-2+: Reindex Workspace** | Force the server to rebuild its workspace index. Displays file and symbol counts on completion. |

## Tasks

The extension provides four tasks via a TaskProvider. Access them from Terminal > Run Task, or the Command Palette > "Tasks: Run Task":

| Task | Command | Description |
|------|---------|-------------|
| build | `m2c build` | Compile the project (requires `m2.toml`) |
| run | `m2c run` | Compile and run |
| test | `m2c test` | Compile and run tests |
| clean | `m2c clean` | Remove `.m2c/` build directory |

The **build** task is assigned to the Build group; **test** to the Test group.

### Problem matcher

Tasks use the `m2c` problem matcher, which parses error output in the format:

```
file:line:col: error: message
```

Errors and warnings from `m2c` are highlighted in the editor and appear in the Problems panel.

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

The LSP server automatically detects projects by looking for `m2.toml` manifest files. When a project is detected:

- Include paths from the manifest and lockfile are used for module resolution
- `m2plus` mode is read from the manifest (overrides the VS Code setting)
- Saving `m2.toml` or `m2.lock` triggers automatic reindexing

No manual `-I` configuration is needed for projects with an `m2.toml` manifest.

## Output and logs

- **Output panel**: Select "Modula-2+ Language Server" from the Output panel dropdown to see LSP server stderr output (errors, warnings, debug messages).
- **Extension log**: Check the Extension Host log for client-side issues.

## Troubleshooting

### Server not starting

1. Verify `m2c` works: run `m2c --version-json` in a terminal.
2. If `m2c` is not on PATH, set `m2c.serverPath` to the full path (e.g., `/usr/local/bin/m2c`).
3. Check the Output panel for error messages.

### No diagnostics

- Ensure the file extension is `.mod` or `.def`.
- Check that the file is syntactically valid enough to parse (the server reports parse errors as diagnostics).
- Try `m2c.diagnostics.debounceMs: 0` for immediate feedback.

### Stale references or symbols

- Save all files, then run "Reindex Workspace" from the command palette.
- If the issue persists after reindexing, restart the language server.

### Extension not activating

- Verify the symlink exists: `ls -la ~/.vscode/extensions/m2plus`
- Ensure the extension compiled: check that `tools/vscode-m2plus/out/extension.js` exists.
- Restart VS Code.

### Build tasks not appearing

- Ensure an `m2.toml` manifest exists in the workspace root.
- Verify `m2c` is accessible (the task provider uses the configured `serverPath`).
