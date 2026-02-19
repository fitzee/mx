import * as vscode from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;

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
        command: "m2c.showDoc",
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
        default:
          this.iconPath = new vscode.ThemeIcon("file");
      }
    } else {
      this.iconPath = new vscode.ThemeIcon("folder");
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
      // Root: show categories
      const categories = [...new Set(this.entries.map(e => e.category))].sort();
      const categoryLabels: Record<string, string> = {
        Language: "Keywords & Constructs",
        Builtin: "Built-in Procedures & Types",
        Stdlib: "Standard Library",
        Extension: "Modula-2+ Extensions",
      };
      return categories.map(cat =>
        new DocTreeItem(
          categoryLabels[cat] ?? cat,
          vscode.TreeItemCollapsibleState.Collapsed,
          undefined,
          cat
        )
      );
    }

    // Children of a category
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

class M2cTaskProvider implements vscode.TaskProvider {
  provideTasks(): vscode.Task[] {
    const config = vscode.workspace.getConfiguration("m2c");
    const m2cPath = config.get<string>("serverPath", "m2c");
    const tasks: vscode.Task[] = [];
    for (const cmd of ["build", "run", "test", "clean", "init"]) {
      const exec = new vscode.ShellExecution(`${m2cPath} ${cmd}`);
      const task = new vscode.Task(
        { type: "m2c", command: cmd },
        vscode.TaskScope.Workspace,
        cmd,
        "m2c",
        exec,
        "$m2c"
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
  const config = vscode.workspace.getConfiguration("m2c");
  const serverPath = config.get<string>("serverPath", "m2c");
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

  client = new LanguageClient(
    "m2c-lsp",
    "Modula-2+ Language Server",
    serverOptions,
    clientOptions
  );

  client.start();

  // Register task provider
  context.subscriptions.push(
    vscode.tasks.registerTaskProvider("m2c", new M2cTaskProvider())
  );

  // Register docs tree view
  const docTree = new DocTreeProvider();
  context.subscriptions.push(
    vscode.window.registerTreeDataProvider("m2cDocsBrowser", docTree)
  );

  // Command: show a single doc in webview (used by tree item click)
  context.subscriptions.push(
    vscode.commands.registerCommand("m2c.showDoc", async (key: string) => {
      if (!client) { return; }
      const doc = await client.sendRequest("m2/getDocumentation", { key });
      const d = doc as { key?: string; markdown?: string } | null;
      if (!d || !d.markdown) {
        vscode.window.showWarningMessage(`No documentation found for ${key}.`);
        return;
      }
      const panel = vscode.window.createWebviewPanel(
        "m2cDoc",
        `M2: ${d.key}`,
        vscode.ViewColumn.One,
        { enableScripts: false }
      );
      panel.webview.html = renderMarkdownHtml(d.key ?? key, d.markdown);
    })
  );

  // Command: refresh docs tree
  context.subscriptions.push(
    vscode.commands.registerCommand("m2c.refreshDocs", () => docTree.refresh())
  );

  // Command: restart server
  context.subscriptions.push(
    vscode.commands.registerCommand("m2c.restartServer", async () => {
      if (client) {
        await client.stop();
        await client.start();
        vscode.window.showInformationMessage("m2c language server restarted");
      }
    })
  );

  // Command: reindex workspace
  context.subscriptions.push(
    vscode.commands.registerCommand("m2c.reindexWorkspace", async () => {
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
    vscode.commands.registerCommand("m2c.initProject", async () => {
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
      const m2cPath = config.get<string>("serverPath", "m2c");
      const arg = name ? ` ${name}` : "";

      try {
        const cp = await import("child_process");
        cp.execSync(`${m2cPath} init${arg}`, { cwd, encoding: "utf-8" });
        vscode.window.showInformationMessage(`Initialized project '${name || "current directory"}'.`);
        vscode.commands.executeCommand("workbench.files.action.refreshFilesExplorer");
      } catch (e: any) {
        const msg = e.stderr?.toString().trim() || e.message;
        vscode.window.showErrorMessage(`m2c init failed: ${msg}`);
      }
    })
  );

  // Command: create debug configuration
  context.subscriptions.push(
    vscode.commands.registerCommand("m2c.createDebugConfig", async () => {
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
          label: "m2c: build debug",
          type: "shell",
          command: "m2c",
          args: ["build", "-g"],
          group: { kind: "build", isDefault: true },
          problemMatcher: "$m2c",
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
            name: "Debug (lldb)",
            type: "lldb",
            request: "launch",
            program: `\${workspaceFolder}/.m2c/bin/${projectName}`,
            args: [],
            cwd: "${workspaceFolder}",
            preLaunchTask: "m2c: build debug",
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
    vscode.commands.registerCommand("m2c.openDocumentation", async () => {
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
        "m2cDoc",
        `M2: ${d.key}`,
        vscode.ViewColumn.Beside,
        { enableScripts: false }
      );
      panel.webview.html = renderMarkdownHtml(d.key ?? pick.label, d.markdown);
    })
  );
}

function renderMarkdownHtml(_title: string, markdown: string): string {
  // Simple markdown-to-HTML conversion for code blocks and headers
  const escaped = markdown
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");

  // Convert ```...``` blocks
  const withCode = escaped.replace(
    /```(\w*)\n([\s\S]*?)```/g,
    '<pre><code class="language-$1">$2</code></pre>'
  );

  // Convert headers
  const withHeaders = withCode
    .replace(/^### (.+)$/gm, "<h3>$1</h3>")
    .replace(/^## (.+)$/gm, "<h2>$1</h2>")
    .replace(/^# (.+)$/gm, "<h1>$1</h1>");

  // Convert bold and inline code
  const withInline = withHeaders
    .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
    .replace(/`([^`]+)`/g, "<code>$1</code>");

  // Convert bullet lists and paragraphs
  const withLists = withInline
    .replace(/^- (.+)$/gm, "<li>$1</li>")
    .replace(/(<li>[\s\S]*?<\/li>)/g, "<ul>$1</ul>");

  // Wrap remaining text in paragraphs
  const lines = withLists.split("\n");
  const html = lines
    .map(l => {
      if (l.startsWith("<") || l.trim() === "") { return l; }
      return `<p>${l}</p>`;
    })
    .join("\n");

  return `<!DOCTYPE html>
<html>
<head>
  <meta charset="UTF-8">
  <style>
    body { font-family: -apple-system, BlinkMacSystemFont, sans-serif; padding: 16px; line-height: 1.6; }
    h1 { border-bottom: 1px solid var(--vscode-panel-border); padding-bottom: 8px; }
    h2 { margin-top: 24px; }
    pre { background: var(--vscode-textCodeBlock-background); padding: 12px; border-radius: 4px; overflow-x: auto; }
    code { font-family: var(--vscode-editor-font-family), monospace; font-size: var(--vscode-editor-font-size); }
    ul { padding-left: 20px; }
  </style>
</head>
<body>${html}</body>
</html>`;
}

export function deactivate(): Thenable<void> | undefined {
  if (!client) {
    return undefined;
  }
  return client.stop();
}
