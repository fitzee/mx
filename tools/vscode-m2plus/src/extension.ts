import * as path from "path";
import * as vscode from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;

// ── Debug Adapter ───────────────────────────────────────────────

class M2DapAdapterFactory implements vscode.DebugAdapterDescriptorFactory {
  createDebugAdapterDescriptor(
    _session: vscode.DebugSession,
    _executable: vscode.DebugAdapterExecutable | undefined
  ): vscode.ProviderResult<vscode.DebugAdapterDescriptor> {
    const config = vscode.workspace.getConfiguration("mx");
    let m2dapPath = config.get<string>("m2dapPath", "");
    if (!m2dapPath) {
      // Resolve m2dap next to mx (e.g. ~/.mx/bin/mx → ~/.mx/bin/m2dap)
      const serverPath = config.get<string>("serverPath", "mx");
      if (serverPath.includes(path.sep)) {
        m2dapPath = path.join(path.dirname(serverPath), "m2dap");
      } else {
        m2dapPath = "m2dap";
      }
    }
    return new vscode.DebugAdapterExecutable(m2dapPath);
  }
}

// ── Documentation Tree View ─────────────────────────────────────

interface DocInfo {
  key: string;
  category: string;
}

class DocTreeItem extends vscode.TreeItem {
  constructor(
    public readonly label: string,
    public readonly collapsibleState: vscode.TreeItemCollapsibleState,
    public readonly docKey?: string,
    public readonly category?: string
  ) {
    super(label, collapsibleState);
    if (docKey) {
      this.command = {
        command: "mx.showDoc",
        title: "Show Documentation",
        arguments: [docKey],
      };
      this.contextValue = "docEntry";
      // Icon based on category
      switch (category) {
        case "Builtin":
          this.iconPath = new vscode.ThemeIcon("symbol-function");
          break;
        case "Language":
          this.iconPath = new vscode.ThemeIcon("symbol-keyword");
          break;
        case "Stdlib":
          this.iconPath = new vscode.ThemeIcon("symbol-module");
          break;
        case "Extension":
          this.iconPath = new vscode.ThemeIcon("extensions");
          break;
        case "LibGraphics":
        case "LibTransport":
        case "LibHTTP":
        case "LibServices":
        case "LibAsync":
        case "LibDatabase":
        case "LibHelpers":
          this.iconPath = new vscode.ThemeIcon("symbol-module");
          break;
        default:
          this.iconPath = new vscode.ThemeIcon("file");
      }
    } else {
      // Category/group node icons
      switch (category) {
        case "_Libraries":
          this.iconPath = new vscode.ThemeIcon("library");
          break;
        case "LibGraphics":
          this.iconPath = new vscode.ThemeIcon("device-desktop");
          break;
        case "LibTransport":
          this.iconPath = new vscode.ThemeIcon("plug");
          break;
        case "LibHTTP":
          this.iconPath = new vscode.ThemeIcon("globe");
          break;
        case "LibServices":
          this.iconPath = new vscode.ThemeIcon("server");
          break;
        case "LibAsync":
          this.iconPath = new vscode.ThemeIcon("sync");
          break;
        case "LibHelpers":
          this.iconPath = new vscode.ThemeIcon("tools");
          break;
        case "LibDatabase":
          this.iconPath = new vscode.ThemeIcon("database");
          break;
        default:
          this.iconPath = new vscode.ThemeIcon("folder");
      }
    }
  }
}

class DocTreeProvider implements vscode.TreeDataProvider<DocTreeItem> {
  private _onDidChangeTreeData = new vscode.EventEmitter<DocTreeItem | undefined>();
  readonly onDidChangeTreeData = this._onDidChangeTreeData.event;

  private entries: DocInfo[] = [];
  private loaded = false;

  refresh(): void {
    this.loaded = false;
    this._onDidChangeTreeData.fire(undefined);
  }

  private async loadEntries(): Promise<void> {
    if (this.loaded || !client) { return; }
    try {
      const result = await client.sendRequest("m2/getDocumentation", {});
      const r = result as { entries?: DocInfo[] };
      this.entries = r.entries ?? [];
      this.loaded = true;
    } catch {
      this.entries = [];
    }
  }

  async getChildren(element?: DocTreeItem): Promise<DocTreeItem[]> {
    await this.loadEntries();

    if (!element) {
      // Root: main categories + Libraries group
      const categories = [...new Set(this.entries.map(e => e.category))].sort();
      const mainCategories = categories.filter(c => !c.startsWith("Lib"));
      const hasLibs = categories.some(c => c.startsWith("Lib"));
      const categoryLabels: Record<string, string> = {
        Language: "Keywords & Constructs",
        Builtin: "Built-in Procedures & Types",
        Stdlib: "Standard Library",
        Extension: "Modula-2+ Extensions",
      };
      const items = mainCategories.map(cat =>
        new DocTreeItem(
          categoryLabels[cat] ?? cat,
          vscode.TreeItemCollapsibleState.Collapsed,
          undefined,
          cat
        )
      );
      if (hasLibs) {
        items.push(new DocTreeItem(
          "Libraries",
          vscode.TreeItemCollapsibleState.Collapsed,
          undefined,
          "_Libraries"
        ));
      }
      return items;
    }

    // Children of Libraries: show subcategories (Graphics, Networking, ...)
    if (element.category === "_Libraries") {
      const libCategories = [...new Set(
        this.entries.filter(e => e.category.startsWith("Lib")).map(e => e.category)
      )].sort();
      const subLabels: Record<string, string> = {
        LibAsync: "Async",
        LibDatabase: "Database",
        LibGraphics: "Graphics",
        LibHTTP: "HTTP",
        LibHelpers: "Helpers",
        LibServices: "Services",
        LibTransport: "Transport",
      };
      return libCategories.map(cat =>
        new DocTreeItem(
          subLabels[cat] ?? cat.replace(/^Lib/, ""),
          vscode.TreeItemCollapsibleState.Collapsed,
          undefined,
          cat
        )
      );
    }

    // Children of a category or subcategory: show doc entries
    if (!element.docKey && element.category) {
      const items = this.entries
        .filter(e => e.category === element.category)
        .sort((a, b) => a.key.localeCompare(b.key));
      return items.map(e =>
        new DocTreeItem(
          e.key,
          vscode.TreeItemCollapsibleState.None,
          e.key,
          e.category
        )
      );
    }

    return [];
  }

  getTreeItem(element: DocTreeItem): vscode.TreeItem {
    return element;
  }
}

// ── Task Provider ───────────────────────────────────────────────

class MxTaskProvider implements vscode.TaskProvider {
  provideTasks(): vscode.Task[] {
    const config = vscode.workspace.getConfiguration("mx");
    const mxPath = config.get<string>("serverPath", "mx");
    const tasks: vscode.Task[] = [];
    for (const cmd of ["build", "run", "test", "clean", "init"]) {
      const exec = new vscode.ShellExecution(`${mxPath} ${cmd}`);
      const task = new vscode.Task(
        { type: "mx", command: cmd },
        vscode.TaskScope.Workspace,
        cmd,
        "mx",
        exec,
        "$mx"
      );
      if (cmd === "build") {
        task.group = vscode.TaskGroup.Build;
      } else if (cmd === "test") {
        task.group = vscode.TaskGroup.Test;
      }
      tasks.push(task);
    }
    return tasks;
  }

  resolveTask(task: vscode.Task): vscode.Task {
    return task;
  }
}

export function activate(context: vscode.ExtensionContext) {
  const config = vscode.workspace.getConfiguration("mx");
  const serverPath = config.get<string>("serverPath", "mx");
  const m2plus = config.get<boolean>("m2plus", true);
  const includePaths = config.get<string[]>("includePaths", []);
  const debounceMs = config.get<number>("diagnostics.debounceMs", 250);

  const args = ["--lsp"];
  if (m2plus) {
    args.push("--m2plus");
  }
  for (const p of includePaths) {
    args.push("-I", p);
  }

  const serverOptions: ServerOptions = {
    command: serverPath,
    args,
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "modula2" }],
    initializationOptions: {
      diagnostics: {
        debounce_ms: debounceMs,
      },
    },
  };

  // Register task provider and docs tree view first (before LSP client)
  // so the UI works even if mx isn't on PATH yet.
  context.subscriptions.push(
    vscode.tasks.registerTaskProvider("mx", new MxTaskProvider())
  );

  // Register m2dap debug adapter
  context.subscriptions.push(
    vscode.debug.registerDebugAdapterDescriptorFactory(
      "m2dap",
      new M2DapAdapterFactory()
    )
  );

  const docTree = new DocTreeProvider();
  context.subscriptions.push(
    vscode.window.registerTreeDataProvider("mxDocsBrowser", docTree)
  );

  client = new LanguageClient(
    "mx-lsp",
    "Modula-2+ Language Server",
    serverOptions,
    clientOptions
  );

  client.start().catch((err: Error) => {
    vscode.window.showWarningMessage(
      `mx language server failed to start: ${err.message}. Ensure mx is on your PATH.`
    );
  });

  // Command: show a single doc in webview (used by tree item click)
  context.subscriptions.push(
    vscode.commands.registerCommand("mx.showDoc", async (key: string) => {
      if (!client) { return; }
      const doc = await client.sendRequest("m2/getDocumentation", { key });
      const d = doc as { key?: string; markdown?: string } | null;
      if (!d || !d.markdown) {
        vscode.window.showWarningMessage(`No documentation found for ${key}.`);
        return;
      }
      const panel = vscode.window.createWebviewPanel(
        "mxDoc",
        `M2: ${d.key}`,
        vscode.ViewColumn.One,
        { enableScripts: false }
      );
      panel.webview.html = renderMarkdownHtml(d.key ?? key, d.markdown);
    })
  );

  // Command: refresh docs tree
  context.subscriptions.push(
    vscode.commands.registerCommand("mx.refreshDocs", () => docTree.refresh())
  );

  // Command: restart server
  context.subscriptions.push(
    vscode.commands.registerCommand("mx.restartServer", async () => {
      if (client) {
        await client.stop();
        await client.start();
        vscode.window.showInformationMessage("mx language server restarted");
      }
    })
  );

  // Command: reindex workspace
  context.subscriptions.push(
    vscode.commands.registerCommand("mx.reindexWorkspace", async () => {
      if (client) {
        const result = await client.sendRequest("m2/reindexWorkspace", {});
        const r = result as { files?: number; symbols?: number };
        vscode.window.showInformationMessage(
          `Reindexed: ${r.files ?? 0} files, ${r.symbols ?? 0} symbols`
        );
      }
    })
  );

  // Command: initialize project
  context.subscriptions.push(
    vscode.commands.registerCommand("mx.initProject", async () => {
      const name = await vscode.window.showInputBox({
        prompt: "Project name",
        placeHolder: "myproject",
      });
      if (name === undefined) { return; } // cancelled

      const folders = vscode.workspace.workspaceFolders;
      if (!folders || folders.length === 0) {
        vscode.window.showErrorMessage("No workspace folder open.");
        return;
      }
      const cwd = folders[0].uri.fsPath;
      const mxPath = config.get<string>("serverPath", "mx");
      const arg = name ? ` ${name}` : "";

      try {
        const cp = await import("child_process");
        cp.execSync(`${mxPath} init${arg}`, { cwd, encoding: "utf-8" });
        vscode.window.showInformationMessage(`Initialized project '${name || "current directory"}'.`);
        vscode.commands.executeCommand("workbench.files.action.refreshFilesExplorer");
      } catch (e: any) {
        const msg = e.stderr?.toString().trim() || e.message;
        vscode.window.showErrorMessage(`mx init failed: ${msg}`);
      }
    })
  );

  // Command: create debug configuration
  context.subscriptions.push(
    vscode.commands.registerCommand("mx.createDebugConfig", async () => {
      const folders = vscode.workspace.workspaceFolders;
      if (!folders || folders.length === 0) {
        vscode.window.showErrorMessage("No workspace folder open.");
        return;
      }
      const root = folders[0].uri;
      const fs = vscode.workspace.fs;

      const tasksContent = JSON.stringify({
        version: "2.0.0",
        tasks: [{
          label: "mx: build debug",
          type: "shell",
          command: "mx",
          args: ["build", "-g"],
          group: { kind: "build", isDefault: true },
          problemMatcher: "$mx",
          presentation: { reveal: "silent", panel: "shared" },
        }],
      }, null, 2);

      // Read project name from m2.toml manifest
      let projectName = folders[0].name; // fallback to folder name
      const manifestUri = vscode.Uri.joinPath(root, "m2.toml");
      try {
        const raw = Buffer.from(await fs.readFile(manifestUri)).toString("utf-8");
        for (const line of raw.split("\n")) {
          const m = line.match(/^name\s*=\s*(.+)/);
          if (m) { projectName = m[1].trim(); break; }
        }
      } catch { /* no manifest yet, use folder name */ }

      const launchContent = JSON.stringify({
        version: "0.2.0",
        configurations: [
          {
            name: "Debug (m2dap)",
            type: "m2dap",
            request: "launch",
            program: `\${workspaceFolder}/.mx/bin/${projectName}`,
            args: [],
            cwd: "${workspaceFolder}",
            stopOnEntry: false,
            preLaunchTask: "mx: build debug",
          },
          {
            name: "Debug (lldb)",
            type: "lldb",
            request: "launch",
            program: `\${workspaceFolder}/.mx/bin/${projectName}`,
            args: [],
            cwd: "${workspaceFolder}",
            preLaunchTask: "mx: build debug",
            sourceLanguages: ["modula2"],
          },
        ],
      }, null, 2);

      const vscodeDirUri = vscode.Uri.joinPath(root, ".vscode");
      await fs.createDirectory(vscodeDirUri);

      const created: string[] = [];

      const tasksUri = vscode.Uri.joinPath(vscodeDirUri, "tasks.json");
      try {
        await fs.stat(tasksUri);
        // File exists — don't overwrite
      } catch {
        await fs.writeFile(tasksUri, Buffer.from(tasksContent, "utf-8"));
        created.push("tasks.json");
      }

      const launchUri = vscode.Uri.joinPath(vscodeDirUri, "launch.json");
      try {
        await fs.stat(launchUri);
      } catch {
        await fs.writeFile(launchUri, Buffer.from(launchContent, "utf-8"));
        created.push("launch.json");
      }

      // Write extensions.json with CodeLLDB recommendation
      const extUri = vscode.Uri.joinPath(vscodeDirUri, "extensions.json");
      try {
        await fs.stat(extUri);
        // Exists — try to merge the recommendation
        const raw = Buffer.from(await fs.readFile(extUri)).toString("utf-8");
        if (!raw.includes("vadimcn.vscode-lldb")) {
          const obj = JSON.parse(raw);
          if (!obj.recommendations) { obj.recommendations = []; }
          obj.recommendations.push("vadimcn.vscode-lldb");
          await fs.writeFile(extUri, Buffer.from(JSON.stringify(obj, null, 2), "utf-8"));
          created.push("extensions.json (updated)");
        }
      } catch {
        const extContent = JSON.stringify({
          recommendations: ["vadimcn.vscode-lldb"],
        }, null, 2);
        await fs.writeFile(extUri, Buffer.from(extContent, "utf-8"));
        created.push("extensions.json");
      }

      // Ensure settings.json has debug.allowBreakpointsEverywhere
      const settingsUri = vscode.Uri.joinPath(vscodeDirUri, "settings.json");
      try {
        const raw = Buffer.from(await fs.readFile(settingsUri)).toString("utf-8");
        if (!raw.includes("debug.allowBreakpointsEverywhere")) {
          const obj = JSON.parse(raw);
          obj["debug.allowBreakpointsEverywhere"] = true;
          await fs.writeFile(settingsUri, Buffer.from(JSON.stringify(obj, null, 2), "utf-8"));
          created.push("settings.json (updated)");
        }
      } catch {
        const settingsContent = JSON.stringify({
          "debug.allowBreakpointsEverywhere": true,
        }, null, 2);
        await fs.writeFile(settingsUri, Buffer.from(settingsContent, "utf-8"));
        created.push("settings.json");
      }

      if (created.length > 0) {
        vscode.window.showInformationMessage(
          `Created .vscode/${created.join(", .vscode/")}. Install the recommended CodeLLDB extension for debugging.`
        );
      } else {
        vscode.window.showInformationMessage(
          "Debug config files already exist. No changes made."
        );
      }
    })
  );

  // Command: open documentation
  context.subscriptions.push(
    vscode.commands.registerCommand("mx.openDocumentation", async () => {
      if (!client) { return; }

      // Fetch all doc entries from the server
      const result = await client.sendRequest("m2/getDocumentation", {});
      const r = result as { entries?: Array<{ key: string; category: string }> };
      if (!r.entries || r.entries.length === 0) {
        vscode.window.showInformationMessage("No documentation entries available.");
        return;
      }

      // Let user pick a topic
      const items = r.entries.map(e => ({
        label: e.key,
        description: e.category,
      }));
      items.sort((a, b) => a.label.localeCompare(b.label));

      const pick = await vscode.window.showQuickPick(items, {
        placeHolder: "Search Modula-2 documentation...",
        matchOnDescription: true,
      });
      if (!pick) { return; }

      // Fetch full markdown for the selected entry
      const doc = await client.sendRequest("m2/getDocumentation", { key: pick.label });
      const d = doc as { key?: string; markdown?: string } | null;
      if (!d || !d.markdown) {
        vscode.window.showWarningMessage(`No documentation found for ${pick.label}.`);
        return;
      }

      // Show in a webview panel
      const panel = vscode.window.createWebviewPanel(
        "mxDoc",
        `M2: ${d.key}`,
        vscode.ViewColumn.Beside,
        { enableScripts: false }
      );
      panel.webview.html = renderMarkdownHtml(d.key ?? pick.label, d.markdown);
    })
  );
}

function renderMarkdownHtml(_title: string, markdown: string): string {
  // 1. Extract code blocks before any other processing
  const codeBlocks: string[] = [];
  let text = markdown.replace(/```(\w*)\n([\s\S]*?)```/g, (_m, lang, code) => {
    const esc = code
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;");
    const idx = codeBlocks.length;
    codeBlocks.push(
      `<pre><code class="language-${lang}">${esc}</code></pre>`
    );
    return `\x00CB${idx}\x00`;
  });

  // 2. HTML-escape the rest
  text = text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");

  // 3. Headers
  text = text
    .replace(/^### (.+)$/gm, "<h3>$1</h3>")
    .replace(/^## (.+)$/gm, "<h2>$1</h2>")
    .replace(/^# (.+)$/gm, "<h1>$1</h1>");

  // 4. Bold and inline code
  text = text
    .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
    .replace(/`([^`]+)`/g, "<code>$1</code>");

  // 5. Horizontal rules
  text = text.replace(/^---$/gm, "<hr>");

  // 6. Tables: header row + separator row + data rows
  text = convertMarkdownTables(text);

  // 7. Bullet lists: group consecutive <li> into <ul>
  text = text.replace(/^- (.+)$/gm, "<li>$1</li>");
  text = text.replace(/((?:<li>.*<\/li>\n?)+)/g, "<ul>$1</ul>");

  // 8. Wrap remaining plain-text lines in <p>
  const lines = text.split("\n");
  const html = lines
    .map(l => {
      const t = l.trim();
      if (t === "" || t.startsWith("<") || t.includes("\x00CB")) { return l; }
      return `<p>${l}</p>`;
    })
    .join("\n");

  // 9. Re-insert code blocks
  let final = html;
  codeBlocks.forEach((block, i) => {
    final = final.replace(`\x00CB${i}\x00`, block);
  });

  return `<!DOCTYPE html>
<html>
<head>
  <meta charset="UTF-8">
  <style>
    body { font-family: -apple-system, BlinkMacSystemFont, sans-serif; padding: 16px; line-height: 1.5; color: var(--vscode-foreground); }
    h1 { border-bottom: 1px solid var(--vscode-panel-border); padding-bottom: 6px; margin-bottom: 12px; }
    h2 { margin-top: 20px; margin-bottom: 8px; }
    h3 { margin-top: 16px; margin-bottom: 6px; }
    p { margin: 6px 0; }
    pre { background: var(--vscode-textCodeBlock-background); padding: 12px; border-radius: 4px; overflow-x: auto; line-height: 1.4; margin: 8px 0; }
    pre code { background: transparent; padding: 0; border-radius: 0; display: block; }
    code { font-family: var(--vscode-editor-font-family), monospace; font-size: var(--vscode-editor-font-size); }
    p code, li code, td code, th code { background: var(--vscode-textCodeBlock-background); padding: 1px 4px; border-radius: 3px; }
    ul { padding-left: 20px; margin: 6px 0; }
    li { margin: 2px 0; }
    table { border-collapse: collapse; margin: 8px 0; }
    th, td { border: 1px solid var(--vscode-panel-border); padding: 4px 10px; text-align: left; }
    th { background: var(--vscode-textCodeBlock-background); font-weight: 600; }
    hr { border: none; border-top: 1px solid var(--vscode-panel-border); margin: 16px 0; }
  </style>
</head>
<body>${final}</body>
</html>`;
}

function convertMarkdownTables(text: string): string {
  const lines = text.split("\n");
  const out: string[] = [];
  let i = 0;
  while (i < lines.length) {
    // Table detected: row starting with |, next line is separator |---|
    if (
      i + 1 < lines.length &&
      lines[i].trim().startsWith("|") &&
      /^\|[\s\-:|]+\|$/.test(lines[i + 1].trim())
    ) {
      const headers = splitTableRow(lines[i]);
      i += 2; // skip header + separator
      let t = "<table><thead><tr>";
      for (const h of headers) { t += `<th>${h}</th>`; }
      t += "</tr></thead><tbody>";
      while (i < lines.length && lines[i].trim().startsWith("|")) {
        const cells = splitTableRow(lines[i]);
        t += "<tr>";
        for (const c of cells) { t += `<td>${c}</td>`; }
        t += "</tr>";
        i++;
      }
      t += "</tbody></table>";
      out.push(t);
    } else {
      out.push(lines[i]);
      i++;
    }
  }
  return out.join("\n");
}

function splitTableRow(line: string): string[] {
  return line.split("|").slice(1, -1).map(c => c.trim());
}

export function deactivate(): Thenable<void> | undefined {
  if (!client) {
    return undefined;
  }
  return client.stop();
}
