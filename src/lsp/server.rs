use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};
use crate::analyze::AnalysisResult;
use crate::json::Json;
use super::transport;
use super::documents::{DocumentStore, uri_to_path, path_to_uri};
use super::analysis::{self, DefCache};
use super::diagnostics;
use super::workspace::{self, ProjectContext};
use super::index::WorkspaceIndex;

/// Default debounce delay for diagnostics (ms).
const DEFAULT_DEBOUNCE_MS: u64 = 250;

/// JSON-RPC error code for request cancelled.
const REQUEST_CANCELLED: i64 = -32800;

/// JSON-RPC error code for invalid request (used after shutdown).
const INVALID_REQUEST: i64 = -32600;

// ── ServerEvent ─────────────────────────────────────────────────────

enum ServerEvent {
    Message(Json),
    Tick,
    StdinClosed,
}

// ── LspServer ───────────────────────────────────────────────────────

pub struct LspServer {
    docs: DocumentStore,
    m2plus: bool,
    include_paths: Vec<PathBuf>,
    initialized: bool,
    shutdown: bool,
    def_cache: DefCache,

    /// Analysis cache: uri → (doc_version, lock_hash, AnalysisResult).
    analysis_cache: HashMap<String, (u64, u64, AnalysisResult)>,

    /// Project context cache: canonical root → ProjectContext.
    project_cache: HashMap<PathBuf, ProjectContext>,

    /// Multi-root workspace roots (from initialize workspaceFolders).
    workspace_roots: Vec<PathBuf>,

    /// Workspace-wide symbol index.
    workspace_index: WorkspaceIndex,

    /// Canceled request IDs (from $/cancelRequest).
    canceled_requests: HashSet<i64>,

    /// Pending diagnostics: URI → last-change instant (for debounce).
    pending_diagnostics: HashMap<String, Instant>,

    /// Pending workspace index updates: URI → last-change instant (for debounce).
    pending_index_updates: HashMap<String, Instant>,

    /// Debounce delay in milliseconds (0 = disabled).
    debounce_ms: u64,

    /// Index update debounce delay in milliseconds (0 = disabled).
    index_debounce_ms: u64,

    /// Counter for server-initiated request IDs.
    next_request_id: i64,

    /// Whether client supports workDoneProgress.
    client_supports_progress: bool,

    /// Channel receiver for the event loop (set during run()).
    event_rx: Option<Receiver<ServerEvent>>,

    /// Messages buffered while waiting for a specific response (e.g. progress create).
    buffered_messages: Vec<Json>,

    /// Library documentation loaded from disk at startup.
    library_docs: crate::lang_docs::LibraryDocs,
}

impl LspServer {
    pub fn new(m2plus: bool, include_paths: Vec<PathBuf>) -> Self {
        let debounce = std::env::var(crate::identity::ENV_LSP_DEBOUNCE)
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_DEBOUNCE_MS);

        let index_debounce = std::env::var(crate::identity::ENV_LSP_INDEX_DEBOUNCE)
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_DEBOUNCE_MS);

        let library_docs = match crate::lang_docs::resolve_docs_root() {
            Some(docs_root) => crate::lang_docs::LibraryDocs::load(&docs_root),
            None => crate::lang_docs::LibraryDocs::empty(),
        };

        Self {
            docs: DocumentStore::new(),
            m2plus,
            include_paths,
            initialized: false,
            shutdown: false,
            def_cache: DefCache::new(),
            analysis_cache: HashMap::new(),
            project_cache: HashMap::new(),
            workspace_roots: Vec::new(),
            workspace_index: WorkspaceIndex::new(),
            canceled_requests: HashSet::new(),
            pending_diagnostics: HashMap::new(),
            pending_index_updates: HashMap::new(),
            debounce_ms: debounce,
            index_debounce_ms: index_debounce,
            next_request_id: 1000,
            client_supports_progress: false,
            event_rx: None,
            buffered_messages: Vec::new(),
            library_docs,
        }
    }

    // ── Cancellation ────────────────────────────────────────────────

    fn is_canceled(&self, id: &Json) -> bool {
        if let Some(n) = id.as_i64() {
            self.canceled_requests.contains(&n)
        } else {
            false
        }
    }

    fn send_canceled(&self, id: &Json) {
        transport::send_error(id, REQUEST_CANCELLED, "Request cancelled");
    }

    fn retire_cancel(&mut self, id: &Json) {
        if let Some(n) = id.as_i64() {
            self.canceled_requests.remove(&n);
        }
    }

    // ── Project context ─────────────────────────────────────────────

    fn get_project_context(&mut self, uri: &str) -> Option<ProjectContext> {
        let file_path = uri_to_path(uri);
        let path = Path::new(&file_path);
        let root = workspace::find_project_root(path)?;
        let canonical = root.canonicalize().unwrap_or_else(|_| root.clone());

        let needs_reload = if let Some(ctx) = self.project_cache.get(&canonical) {
            let lock_path = canonical.join("m2.lock");
            if let Ok(content) = std::fs::read_to_string(&lock_path) {
                workspace::Lockfile::content_hash(&content) != ctx.lock_hash
            } else {
                ctx.lock_hash != 0
            }
        } else {
            true
        };

        if needs_reload {
            if let Some(ctx) = ProjectContext::load(&canonical, &self.include_paths) {
                self.project_cache.insert(canonical.clone(), ctx);
            }
        }

        self.project_cache.get(&canonical).map(|ctx| ProjectContext {
            root: ctx.root.clone(),
            manifest: workspace::Manifest {
                name: ctx.manifest.name.clone(),
                version: ctx.manifest.version.clone(),
                entry: ctx.manifest.entry.clone(),
                m2plus: ctx.manifest.m2plus,
                includes: ctx.manifest.includes.clone(),
                deps: ctx.manifest.deps.iter().map(|d| workspace::DepEntry {
                    name: d.name.clone(),
                    source: match &d.source {
                        workspace::DepSource::Local(p) => workspace::DepSource::Local(p.clone()),
                        workspace::DepSource::Registry(v) => workspace::DepSource::Registry(v.clone()),
                        workspace::DepSource::Installed => workspace::DepSource::Installed,
                    },
                }).collect(),
                cc: workspace::CcSection::default(),
                feature_cc: std::collections::HashMap::new(),
                test: workspace::TestSection::default(),
            },
            lockfile: None,
            include_paths: ctx.include_paths.clone(),
            m2plus: ctx.m2plus,
            lock_hash: ctx.lock_hash,
        })
    }

    fn effective_include_paths(&mut self, uri: &str) -> Vec<PathBuf> {
        if let Some(ctx) = self.get_project_context(uri) {
            ctx.include_paths
        } else {
            self.include_paths.clone()
        }
    }

    fn effective_m2plus(&mut self, uri: &str) -> bool {
        if let Some(ctx) = self.get_project_context(uri) {
            ctx.m2plus
        } else {
            self.m2plus
        }
    }

    fn effective_lock_hash(&mut self, uri: &str) -> u64 {
        if let Some(ctx) = self.get_project_context(uri) {
            ctx.lock_hash
        } else {
            0
        }
    }

    // ── Analysis (always from DocumentStore for open docs) ──────────

    fn get_analysis(&mut self, uri: &str) -> Option<AnalysisResult> {
        let source = self.docs.get(uri)?.to_string();
        let version = self.docs.version(uri);
        let lock_hash = self.effective_lock_hash(uri);

        if let Some((cv, ch, cr)) = self.analysis_cache.get(uri) {
            if *cv == version && *ch == lock_hash {
                return Some(cr.clone());
            }
        }

        let path = uri_to_path(uri);
        let inc_paths = self.effective_include_paths(uri);
        let m2plus = self.effective_m2plus(uri);
        let result = analysis::analyze(&source, &path, m2plus, &inc_paths, &mut self.def_cache);
        self.analysis_cache.insert(uri.to_string(), (version, lock_hash, result.clone()));
        Some(result)
    }

    // ── Workspace indexing ──────────────────────────────────────────

    fn collect_index_dirs(&self) -> Vec<PathBuf> {
        let mut dirs = Vec::new();
        for ctx in self.project_cache.values() {
            for p in &ctx.include_paths {
                if !dirs.contains(p) {
                    dirs.push(p.clone());
                }
            }
        }
        for p in &self.include_paths {
            if !dirs.contains(p) {
                dirs.push(p.clone());
            }
        }
        dirs
    }

    fn initial_index(&mut self) {
        let dirs = self.collect_index_dirs();
        let m2plus = self.m2plus;
        let inc_paths = dirs.clone();

        let token = self.progress_begin("Indexing workspace...");
        let count = self.workspace_index.index_directories(&dirs, m2plus, &inc_paths, &mut self.def_cache);
        self.workspace_index.rebuild_if_dirty();
        self.progress_end(&token, &format!("Indexed {} files ({} symbols)", count, self.workspace_index.symbol_count()));
    }

    fn reindex_workspace(&mut self, force: bool) {
        if force {
            self.workspace_index.force_clear();
        }
        let dirs = self.collect_index_dirs();
        let m2plus = self.m2plus;
        let inc_paths = dirs.clone();

        let token = self.progress_begin("Reindexing workspace...");
        let count = self.workspace_index.index_directories(&dirs, m2plus, &inc_paths, &mut self.def_cache);

        let open_uris: Vec<String> = self.docs.uris().cloned().collect();
        for uri in &open_uris {
            if let Some(source) = self.docs.get(uri).map(|s| s.to_string()) {
                if let Some(result) = self.analysis_cache.get(uri).map(|(_, _, r)| r.clone()) {
                    let path_str = uri_to_path(uri);
                    let path = PathBuf::from(&path_str);
                    self.workspace_index.index_from_analysis(&path, uri, &source, result);
                }
            }
        }

        self.workspace_index.rebuild_if_dirty();
        self.progress_end(&token, &format!("Indexed {} files ({} symbols)", count + open_uris.len(), self.workspace_index.symbol_count()));
    }

    // ── workDoneProgress ────────────────────────────────────────────

    fn progress_begin(&mut self, title: &str) -> String {
        if !self.client_supports_progress {
            return String::new();
        }

        let token = format!("{}-{}", crate::identity::COMPILER_ID, self.next_request_id);
        let create_id = self.next_request_id;
        self.next_request_id += 1;

        // Send create request
        transport::send_request(create_id, "window/workDoneProgress/create",
            Json::obj(vec![("token", Json::str_val(&token))]));

        // Wait for response with timeout (drain channel if available)
        if let Some(rx) = self.event_rx.take() {
            let deadline = Instant::now() + Duration::from_millis(500);
            loop {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() { break; }
                match rx.recv_timeout(remaining) {
                    Ok(ServerEvent::Message(msg)) => {
                        if msg.get("id").and_then(|i| i.as_i64()) == Some(create_id)
                            && msg.get("method").is_none()
                        {
                            if msg.get("error").is_some() {
                                // Client rejected token creation
                                self.event_rx = Some(rx);
                                return String::new();
                            }
                            break;
                        }
                        // Buffer other messages for later processing
                        self.buffered_messages.push(msg);
                    }
                    Ok(ServerEvent::Tick) => {
                        // Skip ticks during progress wait
                    }
                    Ok(ServerEvent::StdinClosed) | Err(_) => break,
                }
            }
            self.event_rx = Some(rx);
        }

        // Send begin notification
        transport::send_notification("$/progress", Json::obj(vec![
            ("token", Json::str_val(&token)),
            ("value", Json::obj(vec![
                ("kind", Json::str_val("begin")),
                ("title", Json::str_val(title)),
                ("cancellable", Json::Bool(false)),
            ])),
        ]));

        token
    }

    fn progress_end(&self, token: &str, message: &str) {
        if token.is_empty() || !self.client_supports_progress {
            return;
        }
        transport::send_notification("$/progress", Json::obj(vec![
            ("token", Json::str_val(token)),
            ("value", Json::obj(vec![
                ("kind", Json::str_val("end")),
                ("message", Json::str_val(message)),
            ])),
        ]));
    }

    // ── Debounced diagnostics ───────────────────────────────────────

    fn flush_pending_diagnostics(&mut self) {
        if self.pending_diagnostics.is_empty() || self.debounce_ms == 0 {
            return;
        }

        let now = Instant::now();
        let threshold = Duration::from_millis(self.debounce_ms);
        let ready: Vec<String> = self.pending_diagnostics.iter()
            .filter(|(_, ts)| now.duration_since(**ts) >= threshold)
            .map(|(uri, _)| uri.clone())
            .collect();

        for uri in &ready {
            self.pending_diagnostics.remove(uri);
            self.analyze_and_publish(uri);
        }
    }

    // ── Debounced index updates ───────────────────────────────────

    fn flush_pending_index_updates(&mut self) {
        if self.pending_index_updates.is_empty() || self.index_debounce_ms == 0 {
            return;
        }

        let now = Instant::now();
        let threshold = Duration::from_millis(self.index_debounce_ms);
        let ready: Vec<String> = self.pending_index_updates.iter()
            .filter(|(_, ts)| now.duration_since(**ts) >= threshold)
            .map(|(uri, _)| uri.clone())
            .collect();

        for uri in &ready {
            self.pending_index_updates.remove(uri);
            self.update_index_for_open_doc(uri);
        }

        if !ready.is_empty() {
            self.workspace_index.rebuild_if_dirty();
        }
    }

    // ── Multi-root routing ──────────────────────────────────────────

    fn root_for_uri(&self, uri: &str) -> Option<PathBuf> {
        let file_path = uri_to_path(uri);
        let mut best: Option<&PathBuf> = None;
        let mut best_len = 0;
        for root in &self.workspace_roots {
            let root_str = root.to_string_lossy();
            if file_path.starts_with(root_str.as_ref()) && root_str.len() > best_len {
                best = Some(root);
                best_len = root_str.len();
            }
        }
        best.cloned()
    }

    // ── Main loop ───────────────────────────────────────────────────

    /// Run the LSP server. Returns exit code: 0 if shutdown was received, 1 otherwise.
    pub fn run(&mut self) -> i32 {
        let (tx, rx) = mpsc::channel::<ServerEvent>();

        // Stdin reader thread: reads JSON-RPC messages and forwards them.
        let stdin_tx = tx.clone();
        std::thread::spawn(move || {
            loop {
                match transport::read_message() {
                    Some(msg) => {
                        if stdin_tx.send(ServerEvent::Message(msg)).is_err() {
                            break;
                        }
                    }
                    None => {
                        let _ = stdin_tx.send(ServerEvent::StdinClosed);
                        break;
                    }
                }
            }
        });

        // Timer thread: ticks every 50ms (configurable via MX_LSP_TICK_MS).
        let tick_ms: u64 = std::env::var(crate::identity::ENV_LSP_TICK)
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(50);
        let timer_tx = tx;
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_millis(tick_ms));
                if timer_tx.send(ServerEvent::Tick).is_err() {
                    break;
                }
            }
        });

        self.event_rx = Some(rx);

        loop {
            // Process any buffered messages first (from progress waits, etc.)
            let buffered = std::mem::take(&mut self.buffered_messages);
            for msg in buffered {
                if self.dispatch_message(&msg) {
                    self.event_rx = None;
                    return if self.shutdown { 0 } else { 1 };
                }
            }

            // Take rx out to avoid borrow conflicts, recv, then put back.
            let rx = match self.event_rx.take() {
                Some(r) => r,
                None => break,
            };
            let event = rx.recv();
            self.event_rx = Some(rx);

            match event {
                Ok(ServerEvent::Message(msg)) => {
                    if self.dispatch_message(&msg) {
                        break;
                    }
                }
                Ok(ServerEvent::Tick) => {
                    self.flush_pending_diagnostics();
                    self.flush_pending_index_updates();
                }
                Ok(ServerEvent::StdinClosed) | Err(_) => break,
            }
        }

        self.event_rx = None;
        if self.shutdown { 0 } else { 1 }
    }

    // ── Message dispatch ────────────────────────────────────────────

    /// Dispatch a single JSON-RPC message. Returns true if server should exit.
    fn dispatch_message(&mut self, msg: &Json) -> bool {
        let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let id = msg.get("id");
        let params = msg.get("params");

        // Protocol hygiene: after shutdown, only accept "exit".
        if self.shutdown {
            if method == "exit" {
                return true;
            }
            if let Some(id) = id {
                transport::send_error(id, INVALID_REQUEST, "Server is shutting down");
            }
            return false;
        }

        match method {
            // ── Lifecycle ────────────────────────────────────
            "initialize" => {
                if let (Some(id), Some(p)) = (id, params) {
                    self.handle_initialize(id, p);
                } else if let Some(id) = id {
                    self.handle_initialize(id, &Json::Null);
                }
            }
            "initialized" => {
                self.initialized = true;
                self.initial_index();
            }
            "shutdown" => {
                self.shutdown = true;
                if let Some(id) = id {
                    transport::send_response(id, Json::Null);
                }
            }
            "exit" => return true,

            // ── Cancellation ─────────────────────────────────
            "$/cancelRequest" => {
                if let Some(p) = params {
                    if let Some(cancel_id) = p.get("id").and_then(|i| i.as_i64()) {
                        self.canceled_requests.insert(cancel_id);
                    }
                }
            }

            // ── Document sync ────────────────────────────────
            "textDocument/didOpen" => {
                if let Some(p) = params { self.handle_did_open(p); }
            }
            "textDocument/didChange" => {
                if let Some(p) = params { self.handle_did_change(p); }
            }
            "textDocument/didClose" => {
                if let Some(p) = params { self.handle_did_close(p); }
            }
            "textDocument/didSave" => {
                if let Some(p) = params { self.handle_did_save(p); }
            }

            // ── Request handlers ─────────────────────────────
            "textDocument/documentSymbol" => {
                if let (Some(id), Some(p)) = (id, params) {
                    if self.is_canceled(id) { self.send_canceled(id); self.retire_cancel(id); }
                    else { self.handle_document_symbol(id, p); }
                }
            }
            "textDocument/hover" => {
                if let (Some(id), Some(p)) = (id, params) {
                    if self.is_canceled(id) { self.send_canceled(id); self.retire_cancel(id); }
                    else { self.handle_hover(id, p); }
                }
            }
            "textDocument/definition" => {
                if let (Some(id), Some(p)) = (id, params) {
                    if self.is_canceled(id) { self.send_canceled(id); self.retire_cancel(id); }
                    else { self.handle_goto_definition(id, p); }
                }
            }
            "textDocument/completion" => {
                if let (Some(id), Some(p)) = (id, params) {
                    if self.is_canceled(id) { self.send_canceled(id); self.retire_cancel(id); }
                    else { self.handle_completion(id, p); }
                }
            }
            "completionItem/resolve" => {
                if let (Some(id), Some(p)) = (id, params) {
                    if self.is_canceled(id) { self.send_canceled(id); self.retire_cancel(id); }
                    else { self.handle_completion_resolve(id, p); }
                }
            }
            "textDocument/rename" => {
                if let (Some(id), Some(p)) = (id, params) {
                    if self.is_canceled(id) { self.send_canceled(id); self.retire_cancel(id); }
                    else { self.handle_rename(id, p); }
                }
            }
            "textDocument/prepareRename" => {
                if let (Some(id), Some(p)) = (id, params) {
                    if self.is_canceled(id) { self.send_canceled(id); self.retire_cancel(id); }
                    else { self.handle_prepare_rename(id, p); }
                }
            }
            "textDocument/references" => {
                if let (Some(id), Some(p)) = (id, params) {
                    if self.is_canceled(id) { self.send_canceled(id); self.retire_cancel(id); }
                    else { self.handle_references(id, p); }
                }
            }
            "textDocument/documentHighlight" => {
                if let (Some(id), Some(p)) = (id, params) {
                    if self.is_canceled(id) { self.send_canceled(id); self.retire_cancel(id); }
                    else { self.handle_document_highlight(id, p); }
                }
            }
            "textDocument/signatureHelp" => {
                if let (Some(id), Some(p)) = (id, params) {
                    if self.is_canceled(id) { self.send_canceled(id); self.retire_cancel(id); }
                    else { self.handle_signature_help(id, p); }
                }
            }
            "textDocument/semanticTokens/full" => {
                if let (Some(id), Some(p)) = (id, params) {
                    if self.is_canceled(id) { self.send_canceled(id); self.retire_cancel(id); }
                    else { self.handle_semantic_tokens(id, p); }
                }
            }
            "textDocument/codeAction" => {
                if let (Some(id), Some(p)) = (id, params) {
                    if self.is_canceled(id) { self.send_canceled(id); self.retire_cancel(id); }
                    else { self.handle_code_action(id, p); }
                }
            }
            "textDocument/prepareCallHierarchy" => {
                if let (Some(id), Some(p)) = (id, params) {
                    if self.is_canceled(id) { self.send_canceled(id); self.retire_cancel(id); }
                    else { self.handle_prepare_call_hierarchy(id, p); }
                }
            }
            "callHierarchy/incomingCalls" => {
                if let (Some(id), Some(p)) = (id, params) {
                    if self.is_canceled(id) { self.send_canceled(id); self.retire_cancel(id); }
                    else { self.handle_incoming_calls(id, p); }
                }
            }
            "callHierarchy/outgoingCalls" => {
                if let (Some(id), Some(p)) = (id, params) {
                    if self.is_canceled(id) { self.send_canceled(id); self.retire_cancel(id); }
                    else { self.handle_outgoing_calls(id, p); }
                }
            }
            "workspace/symbol" => {
                if let (Some(id), Some(p)) = (id, params) {
                    if self.is_canceled(id) { self.send_canceled(id); self.retire_cancel(id); }
                    else { self.handle_workspace_symbol(id, p); }
                }
            }

            // ── Custom: reindex ──────────────────────────────
            "m2/reindexWorkspace" => {
                if let Some(id) = id {
                    if self.is_canceled(id) { self.send_canceled(id); self.retire_cancel(id); }
                    else { self.handle_reindex(id); }
                }
            }

            // ── Custom: getDocumentation ────────────────────
            "m2/getDocumentation" => {
                if let Some(id) = id {
                    let empty = Json::obj(vec![]);
                    let p = params.unwrap_or(&empty);
                    self.handle_get_documentation(id, p);
                }
            }

            // ── Responses to our requests (e.g. progress create) ─
            "" if id.is_some() => {
                // Response to a server-initiated request; ignore.
            }

            _ => {
                if let Some(id) = id {
                    transport::send_error(id, -32601, &format!("method not found: {}", method));
                }
            }
        }

        false
    }

    // ── initialize ──────────────────────────────────────────────────

    fn handle_initialize(&mut self, id: &Json, params: &Json) {
        if let Some(caps) = params.get("capabilities") {
            if let Some(window) = caps.get("window") {
                if let Some(Json::Bool(true)) = window.get("workDoneProgress") {
                    self.client_supports_progress = true;
                }
            }
        }

        if let Some(opts) = params.get("initializationOptions") {
            if let Some(diag) = opts.get("diagnostics") {
                if let Some(ms) = diag.get("debounce_ms").and_then(|v| v.as_i64()) {
                    self.debounce_ms = ms as u64;
                }
                if let Some(ms) = diag.get("index_debounce_ms").and_then(|v| v.as_i64()) {
                    self.index_debounce_ms = ms as u64;
                }
            }
        }

        // Multi-root: workspaceFolders
        if let Some(folders) = params.get("workspaceFolders").and_then(|f| f.as_array()) {
            for folder in folders {
                if let Some(folder_uri) = folder.get("uri").and_then(|u| u.as_str()) {
                    let folder_path = uri_to_path(folder_uri);
                    let root = PathBuf::from(&folder_path);
                    let canonical = root.canonicalize().unwrap_or_else(|_| root.clone());
                    if canonical.join("m2.toml").exists() {
                        if let Some(ctx) = ProjectContext::load(&canonical, &self.include_paths) {
                            if ctx.m2plus { self.m2plus = true; }
                            self.project_cache.insert(canonical.clone(), ctx);
                        }
                    }
                    self.workspace_roots.push(canonical);
                }
            }
        }

        // Fallback: rootUri
        if self.workspace_roots.is_empty() {
            if let Some(root_uri) = params.get("rootUri").and_then(|u| u.as_str()) {
                let root_path = uri_to_path(root_uri);
                let root = PathBuf::from(&root_path);
                let canonical = root.canonicalize().unwrap_or_else(|_| root.clone());
                if canonical.join("m2.toml").exists() {
                    if let Some(ctx) = ProjectContext::load(&canonical, &self.include_paths) {
                        self.m2plus = ctx.m2plus;
                        self.project_cache.insert(canonical.clone(), ctx);
                    }
                }
                self.workspace_roots.push(canonical);
            }
        }

        let capabilities = Json::obj(vec![
            ("textDocumentSync", Json::obj(vec![
                ("openClose", Json::Bool(true)),
                ("change", Json::int_val(1)), // Full sync
                ("save", Json::obj(vec![
                    ("includeText", Json::Bool(true)),
                ])),
            ])),
            ("documentSymbolProvider", Json::Bool(true)),
            ("hoverProvider", Json::Bool(true)),
            ("definitionProvider", Json::Bool(true)),
            ("completionProvider", Json::obj(vec![
                ("triggerCharacters", Json::arr(vec![Json::str_val(".")])),
                ("resolveProvider", Json::Bool(true)),
            ])),
            ("signatureHelpProvider", Json::obj(vec![
                ("triggerCharacters", Json::arr(vec![
                    Json::str_val("("),
                    Json::str_val(","),
                ])),
            ])),
            ("renameProvider", Json::obj(vec![
                ("prepareProvider", Json::Bool(true)),
            ])),
            ("referencesProvider", Json::Bool(true)),
            ("documentHighlightProvider", Json::Bool(true)),
            ("workspaceSymbolProvider", Json::Bool(true)),
            ("semanticTokensProvider", Json::obj(vec![
                ("legend", Json::obj(vec![
                    ("tokenTypes", Json::arr(super::semantic_tokens::token_types_legend())),
                    ("tokenModifiers", Json::arr(Vec::new())),
                ])),
                ("full", Json::Bool(true)),
            ])),
            ("codeActionProvider", Json::Bool(true)),
            ("callHierarchyProvider", Json::Bool(true)),
        ]);

        let result = Json::obj(vec![
            ("capabilities", capabilities),
            ("serverInfo", Json::obj(vec![
                ("name", Json::str_val("mx-lsp")),
                ("version", Json::str_val(env!("CARGO_PKG_VERSION"))),
            ])),
        ]);

        transport::send_response(id, result);
    }

    // ── Document sync ───────────────────────────────────────────────

    fn handle_did_open(&mut self, params: &Json) {
        if let Some(td) = params.get("textDocument") {
            let uri = td.get("uri").and_then(|u| u.as_str()).unwrap_or("");
            let text = td.get("text").and_then(|t| t.as_str()).unwrap_or("");
            self.docs.open(uri, text.to_string());
            self.analyze_and_publish(uri);
            self.update_index_for_open_doc(uri);
        }
    }

    fn handle_did_change(&mut self, params: &Json) {
        if let Some(td) = params.get("textDocument") {
            let uri = td.get("uri").and_then(|u| u.as_str()).unwrap_or("");
            if let Some(changes) = params.get("contentChanges").and_then(|c| c.as_array()) {
                // Sync validation: we advertise full sync (change=1).
                // If client sends incremental (with range), log and use last entry.
                if let Some(first) = changes.first() {
                    if first.get("range").is_some() {
                        eprintln!("mx-lsp: incremental change received but server uses full sync; using last entry");
                    }
                }

                if let Some(last) = changes.last() {
                    let text = last.get("text").and_then(|t| t.as_str()).unwrap_or("");
                    self.docs.change(uri, text.to_string());
                    self.analysis_cache.remove(uri);

                    if self.debounce_ms == 0 {
                        self.analyze_and_publish(uri);
                        self.update_index_for_open_doc(uri);
                        self.workspace_index.rebuild_if_dirty();
                    } else {
                        self.pending_diagnostics.insert(uri.to_string(), Instant::now());
                        self.pending_index_updates.insert(uri.to_string(), Instant::now());
                    }
                }
            }
        }
    }

    fn handle_did_close(&mut self, params: &Json) {
        if let Some(td) = params.get("textDocument") {
            let uri = td.get("uri").and_then(|u| u.as_str()).unwrap_or("");
            self.docs.close(uri);
            self.pending_diagnostics.remove(uri);
            self.pending_index_updates.remove(uri);
            transport::send_notification(
                "textDocument/publishDiagnostics",
                diagnostics::publish_diagnostics(uri, Vec::new()),
            );
        }
    }

    fn handle_did_save(&mut self, params: &Json) {
        if let Some(td) = params.get("textDocument") {
            let uri = td.get("uri").and_then(|u| u.as_str()).unwrap_or("");
            if let Some(text) = params.get("text").and_then(|t| t.as_str()) {
                self.docs.change(uri, text.to_string());
            }

            self.pending_diagnostics.remove(uri);
            self.pending_index_updates.remove(uri);

            let file_path = uri_to_path(uri);

            // Manifest/lockfile save: evict + reindex
            if file_path.ends_with("m2.toml") || file_path.ends_with("m2.lock") {
                let path = Path::new(&file_path);
                if let Some(parent) = path.parent() {
                    let canonical = parent.canonicalize().unwrap_or_else(|_| parent.to_path_buf());
                    self.project_cache.remove(&canonical);
                    let prefix = format!("file://{}", canonical.display());
                    let uris_to_remove: Vec<String> = self.analysis_cache.keys()
                        .filter(|k| k.starts_with(&prefix))
                        .cloned()
                        .collect();
                    for k in &uris_to_remove {
                        self.analysis_cache.remove(k);
                    }
                    let open_uris: Vec<String> = self.docs.uris()
                        .filter(|u| u.starts_with(&prefix))
                        .cloned()
                        .collect();
                    for open_uri in &open_uris {
                        self.analyze_and_publish(open_uri);
                    }
                    self.reindex_workspace(false);
                }
                return;
            }

            // Re-index saved .mod/.def file
            if file_path.ends_with(".mod") || file_path.ends_with(".def")
                || file_path.ends_with(".MOD") || file_path.ends_with(".DEF")
            {
                let path = PathBuf::from(&file_path);
                let inc_paths = self.effective_include_paths(uri);
                let m2plus = self.effective_m2plus(uri);
                self.workspace_index.index_file(&path, m2plus, &inc_paths, &mut self.def_cache);
            }

            self.analyze_and_publish(uri);
        }
    }

    // ── Request handlers ────────────────────────────────────────────

    fn handle_document_symbol(&mut self, id: &Json, params: &Json) {
        let uri = extract_uri(params);
        if let Some(result) = self.get_analysis(&uri) {
            if let Some(ref unit) = result.ast {
                let syms = super::symbols::document_symbols(unit);
                transport::send_response(id, Json::arr(syms));
                return;
            }
        }
        transport::send_response(id, Json::arr(Vec::new()));
    }

    fn handle_hover(&mut self, id: &Json, params: &Json) {
        let uri = extract_uri(params);
        let (line, col) = extract_position(params);

        if let Some(source) = self.docs.get(&uri).map(|s| s.to_string()) {
            if let Some(result) = self.get_analysis(&uri) {
                if let Some(hover) = super::hover::hover(
                    &source, line, col, &result.symtab, &result.types, &result.scope_map,
                ) {
                    transport::send_response(id, hover);
                    return;
                }
            }
        }
        transport::send_response(id, Json::Null);
    }

    fn handle_goto_definition(&mut self, id: &Json, params: &Json) {
        let uri = extract_uri(params);
        let (line, col) = extract_position(params);
        let inc_paths = self.effective_include_paths(&uri);

        if let Some(source) = self.docs.get(&uri).map(|s| s.to_string()) {
            if let Some(result) = self.get_analysis(&uri) {
                if let Some(loc) = super::goto_def::goto_definition(
                    &source, &uri, line, col, &result.symtab, &inc_paths,
                ) {
                    transport::send_response(id, loc);
                    return;
                }
            }
        }
        transport::send_response(id, Json::Null);
    }

    fn handle_completion(&mut self, id: &Json, params: &Json) {
        let uri = extract_uri(params);
        let (line, col) = extract_position(params);

        if let Some(source) = self.docs.get(&uri).map(|s| s.to_string()) {
            if let Some(result) = self.get_analysis(&uri) {
                let completions = super::completion::completion(
                    &source, &uri, line, col, &result.symtab, &result.types,
                    &result.scope_map, Some(&self.workspace_index),
                );
                transport::send_response(id, completions);
                return;
            }
        }
        transport::send_response(id, Json::obj(vec![
            ("isIncomplete", Json::Bool(false)),
            ("items", Json::arr(Vec::new())),
        ]));
    }

    fn handle_completion_resolve(&mut self, id: &Json, params: &Json) {
        if let Some(data) = params.get("data") {
            let uri = data.get("uri").and_then(|u| u.as_str()).unwrap_or("");
            let name = data.get("name").and_then(|n| n.as_str()).unwrap_or("");
            if !uri.is_empty() && !name.is_empty() {
                if let Some(result) = self.get_analysis(uri) {
                    let resolved = super::completion::resolve_completion(
                        name, &result.symtab, &result.types, params.clone(),
                    );
                    transport::send_response(id, resolved);
                    return;
                }
            }
        }
        transport::send_response(id, params.clone());
    }

    fn handle_rename(&mut self, id: &Json, params: &Json) {
        let uri = extract_uri(params);
        let (line, col) = extract_position(params);
        let new_name = params.get("newName").and_then(|n| n.as_str()).unwrap_or("");

        if let Some(result) = self.get_analysis(&uri) {
            // Find symbol at cursor
            let target = match result.ref_index.at_position(line, col) {
                Some(t) => t.clone(),
                None => {
                    transport::send_response(id, Json::Null);
                    return;
                }
            };

            // Same-file edits from ReferenceIndex
            let same_file_refs = result.ref_index.find_all(target.def_scope, &target.name);
            let mut changes: HashMap<String, Vec<Json>> = HashMap::new();
            let same_edits: Vec<Json> = same_file_refs.iter().map(|r| {
                make_text_edit(r.line, r.col, r.len, new_name)
            }).collect();
            if !same_edits.is_empty() {
                changes.insert(uri.clone(), same_edits);
            }

            // Cross-file edits via identity-based index
            let file_path = uri_to_path(&uri);
            let identity_key = super::index::resolve_identity(&result.symtab, &target.name, &file_path)
                .map(|i| i.key());

            if let Some(ref key) = identity_key {
                let workspace_root = self.root_for_uri(&uri);
                self.workspace_index.rebuild_if_dirty();
                let cross_refs: Vec<super::index::IdentityRef> =
                    self.workspace_index.find_refs_by_identity(key).to_vec();

                for cr in &cross_refs {
                    if cr.file_uri == uri { continue; }

                    // Workspace root filter: skip locations outside workspace root
                    if let Some(ref root) = workspace_root {
                        let file_path = uri_to_path(&cr.file_uri);
                        let root_str = root.to_string_lossy();
                        if !file_path.starts_with(root_str.as_ref()) {
                            eprintln!("mx-lsp: rename skipping {} (outside workspace root {})",
                                cr.file_uri, root_str);
                            continue;
                        }
                    }

                    changes.entry(cr.file_uri.clone())
                        .or_default()
                        .push(make_text_edit(cr.line, cr.col, cr.len, new_name));
                }
            }

            if !changes.is_empty() {
                let changes_entries: Vec<(&str, Json)> = changes.iter()
                    .map(|(u, eds)| (u.as_str(), Json::arr(eds.clone())))
                    .collect();
                transport::send_response(id, Json::obj(vec![
                    ("changes", Json::obj(changes_entries)),
                ]));
                return;
            }
        }
        transport::send_response(id, Json::Null);
    }

    fn handle_prepare_rename(&mut self, id: &Json, params: &Json) {
        let uri = extract_uri(params);
        let (line, col) = extract_position(params);

        if let Some(result) = self.get_analysis(&uri) {
            if let Some(prep) = super::prepare_rename::prepare_rename(line, col, &result.ref_index) {
                transport::send_response(id, prep);
                return;
            }
        }
        transport::send_response(id, Json::Null);
    }

    fn handle_references(&mut self, id: &Json, params: &Json) {
        let uri = extract_uri(params);
        let (line, col) = extract_position(params);

        if let Some(result) = self.get_analysis(&uri) {
            let mut locations = Vec::new();
            if let Some(target) = result.ref_index.at_position(line, col) {
                let name = target.name.clone();
                let def_scope = target.def_scope;

                // Same-file refs
                for r in result.ref_index.find_all(def_scope, &name) {
                    locations.push(make_location(&uri, r.line, r.col, r.len));
                }

                // Cross-file refs: prefer identity-based, fallback to name-based
                let ref_file = uri_to_path(&uri);
                let identity_key = super::index::resolve_identity(&result.symtab, &name, &ref_file)
                    .map(|i| i.key());
                let workspace_root = self.root_for_uri(&uri);

                self.workspace_index.rebuild_if_dirty();

                if let Some(ref key) = identity_key {
                    let cross_refs: Vec<super::index::IdentityRef> =
                        self.workspace_index.find_refs_by_identity(key).to_vec();
                    for cr in &cross_refs {
                        if cr.file_uri == uri { continue; }
                        // Limit to workspace root
                        if let Some(ref root) = workspace_root {
                            let file_path = uri_to_path(&cr.file_uri);
                            if !file_path.starts_with(&root.to_string_lossy().as_ref()) {
                                continue;
                            }
                        }
                        locations.push(make_location(&cr.file_uri, cr.line, cr.col, cr.len));
                    }
                } else {
                    // Fallback: name-based
                    let module_name = self.detect_module_name(&uri);
                    if let Some(ref mod_name) = module_name {
                        let cross_refs = self.workspace_index.find_cross_file_refs(mod_name, &name);
                        for cr in &cross_refs {
                            if cr.file_uri == uri { continue; }
                            locations.push(make_location(&cr.file_uri, cr.line, cr.col, cr.len));
                        }
                    }
                }
            }

            if !locations.is_empty() {
                transport::send_response(id, Json::arr(locations));
                return;
            }
        }
        transport::send_response(id, Json::arr(Vec::new()));
    }

    fn handle_document_highlight(&mut self, id: &Json, params: &Json) {
        let uri = extract_uri(params);
        let (line, col) = extract_position(params);

        if let Some(result) = self.get_analysis(&uri) {
            if let Some(highlights) = super::highlight::document_highlight(line, col, &result.ref_index) {
                transport::send_response(id, highlights);
                return;
            }
        }
        transport::send_response(id, Json::arr(Vec::new()));
    }

    fn handle_signature_help(&mut self, id: &Json, params: &Json) {
        let uri = extract_uri(params);
        let (line, col) = extract_position(params);

        if let Some(source) = self.docs.get(&uri).map(|s| s.to_string()) {
            if let Some(result) = self.get_analysis(&uri) {
                if let Some(sig) = super::signature_help::signature_help(
                    &source, line, col, &result.symtab, &result.types, &result.scope_map,
                ) {
                    transport::send_response(id, sig);
                    return;
                }
            }
        }
        transport::send_response(id, Json::Null);
    }

    fn handle_workspace_symbol(&mut self, id: &Json, params: &Json) {
        let query = params.get("query").and_then(|q| q.as_str()).unwrap_or("");

        self.workspace_index.rebuild_if_dirty();
        let results = self.workspace_index.search(query, 200);

        let symbols: Vec<Json> = results.iter().map(|sym| {
            let kind = sym.kind.to_lsp_kind();
            let line = if sym.line > 0 { sym.line - 1 } else { 0 };
            let col = if sym.col > 0 { sym.col - 1 } else { 0 };
            let range = Json::obj(vec![
                ("start", Json::obj(vec![
                    ("line", Json::int_val(line as i64)),
                    ("character", Json::int_val(col as i64)),
                ])),
                ("end", Json::obj(vec![
                    ("line", Json::int_val(line as i64)),
                    ("character", Json::int_val((col + sym.name.len()) as i64)),
                ])),
            ]);
            let mut fields = vec![
                ("name", Json::str_val(&sym.name)),
                ("kind", Json::int_val(kind)),
                ("location", Json::obj(vec![
                    ("uri", Json::str_val(&sym.file_uri)),
                    ("range", range),
                ])),
            ];
            if let Some(ref container) = sym.container {
                fields.push(("containerName", Json::str_val(container)));
            }
            Json::obj(fields)
        }).collect();

        transport::send_response(id, Json::arr(symbols));
    }

    fn handle_semantic_tokens(&mut self, id: &Json, params: &Json) {
        let uri = extract_uri(params);
        if let Some(source) = self.docs.get(&uri).map(|s| s.to_string()) {
            if let Some(result) = self.get_analysis(&uri) {
                let path = uri_to_path(&uri);
                let m2plus = self.effective_m2plus(&uri);
                let data = super::semantic_tokens::collect_semantic_tokens(&source, &path, m2plus, &result);
                transport::send_response(id, super::semantic_tokens::semantic_tokens_response(data));
                return;
            }
        }
        transport::send_response(id, Json::obj(vec![("data", Json::arr(Vec::new()))]));
    }

    fn handle_code_action(&mut self, id: &Json, params: &Json) {
        let uri = extract_uri(params);
        if let Some(source) = self.docs.get(&uri).map(|s| s.to_string()) {
            if let Some(result) = self.get_analysis(&uri) {
                self.workspace_index.rebuild_if_dirty();
                let actions = super::code_actions::code_actions(
                    &uri, &source, params, &result, &self.workspace_index,
                );
                transport::send_response(id, Json::arr(actions));
                return;
            }
        }
        transport::send_response(id, Json::arr(Vec::new()));
    }

    fn handle_prepare_call_hierarchy(&mut self, id: &Json, params: &Json) {
        let uri = extract_uri(params);
        let (line, col) = extract_position(params);
        if let Some(source) = self.docs.get(&uri).map(|s| s.to_string()) {
            if let Some(result) = self.get_analysis(&uri) {
                let items = super::call_hierarchy::prepare_call_hierarchy(
                    &source, &uri, line, col, &result,
                );
                transport::send_response(id, Json::arr(items));
                return;
            }
        }
        transport::send_response(id, Json::arr(Vec::new()));
    }

    fn handle_incoming_calls(&mut self, id: &Json, params: &Json) {
        if let Some(item) = params.get("item") {
            let uri = item.get("uri").and_then(|u| u.as_str()).unwrap_or("");
            let name = item.get("name").and_then(|n| n.as_str()).unwrap_or("");

            // Extract identity key from data (set by prepare_call_hierarchy)
            let identity_key = item.get("data")
                .and_then(|d| d.get("identityKey"))
                .and_then(|k| k.as_str())
                .unwrap_or("");

            // Rebuild workspace index if dirty (non-blocking best-effort)
            self.workspace_index.rebuild_if_dirty();

            // Cancellation check after rebuild (may have taken time)
            if self.is_canceled(id) {
                self.send_canceled(id);
                self.retire_cancel(id);
                return;
            }

            // Root scoping: filter edges to workspace root
            let workspace_root = self.root_for_uri(uri);

            // Try workspace-wide first, then single-file fallback
            let single_file = if !uri.is_empty() { self.get_analysis(uri) } else { None };
            let mut calls = super::call_hierarchy::incoming_calls_ws(
                name, identity_key, &self.workspace_index, single_file.as_ref(), uri,
            );

            // Root scoping filter (with cancellation check)
            if let Some(ref root) = workspace_root {
                if self.is_canceled(id) {
                    self.send_canceled(id);
                    self.retire_cancel(id);
                    return;
                }
                let root_str = root.to_string_lossy();
                calls.retain(|c| {
                    let from_uri = c.get("from")
                        .and_then(|f| f.get("uri"))
                        .and_then(|u| u.as_str())
                        .unwrap_or("");
                    if from_uri.is_empty() { return true; }
                    let file_path = uri_to_path(from_uri);
                    file_path.starts_with(root_str.as_ref())
                });
            }

            transport::send_response(id, Json::arr(calls));
            return;
        }
        transport::send_response(id, Json::arr(Vec::new()));
    }

    fn handle_outgoing_calls(&mut self, id: &Json, params: &Json) {
        if let Some(item) = params.get("item") {
            let uri = item.get("uri").and_then(|u| u.as_str()).unwrap_or("");
            let name = item.get("name").and_then(|n| n.as_str()).unwrap_or("");

            let identity_key = item.get("data")
                .and_then(|d| d.get("identityKey"))
                .and_then(|k| k.as_str())
                .unwrap_or("");

            self.workspace_index.rebuild_if_dirty();

            // Cancellation check after rebuild
            if self.is_canceled(id) {
                self.send_canceled(id);
                self.retire_cancel(id);
                return;
            }

            let workspace_root = self.root_for_uri(uri);

            let single_file = if !uri.is_empty() { self.get_analysis(uri) } else { None };
            let mut calls = super::call_hierarchy::outgoing_calls_ws(
                name, identity_key, &self.workspace_index, single_file.as_ref(), uri,
            );

            // Root scoping filter (with cancellation check)
            if let Some(ref root) = workspace_root {
                if self.is_canceled(id) {
                    self.send_canceled(id);
                    self.retire_cancel(id);
                    return;
                }
                let root_str = root.to_string_lossy();
                calls.retain(|c| {
                    let to_uri = c.get("to")
                        .and_then(|f| f.get("uri"))
                        .and_then(|u| u.as_str())
                        .unwrap_or("");
                    if to_uri.is_empty() { return true; }
                    let file_path = uri_to_path(to_uri);
                    file_path.starts_with(root_str.as_ref())
                });
            }

            transport::send_response(id, Json::arr(calls));
            return;
        }
        transport::send_response(id, Json::arr(Vec::new()));
    }

    fn handle_reindex(&mut self, id: &Json) {
        self.reindex_workspace(true);
        transport::send_response(id, Json::obj(vec![
            ("files", Json::int_val(self.workspace_index.file_count() as i64)),
            ("symbols", Json::int_val(self.workspace_index.symbol_count() as i64)),
        ]));
    }

    fn handle_get_documentation(&mut self, id: &Json, params: &Json) {
        let key = params.get("key")
            .and_then(|k| k.as_str())
            .unwrap_or("");

        if key.is_empty() {
            // List all available documentation entries — core + library + stdlib procs
            let mut entries: Vec<Json> = crate::lang_docs::all_keys()
                .iter()
                .map(|k| {
                    let entry = crate::lang_docs::get_doc(k).unwrap();
                    Json::obj(vec![
                        ("key", Json::str_val(entry.key)),
                        ("category", Json::str_val(&format!("{:?}", entry.category))),
                    ])
                })
                .collect();

            // Library docs (loaded from disk)
            for k in self.library_docs.all_keys() {
                if let Some(entry) = self.library_docs.get(k) {
                    entries.push(Json::obj(vec![
                        ("key", Json::str_val(&entry.key)),
                        ("category", Json::str_val(&entry.category)),
                    ]));
                }
            }

            // Individual stdlib procedure/variable entries
            let mut seen = std::collections::HashSet::new();
            for (module, name, _sig, _doc) in crate::stdlib::stdlib_all_proc_docs() {
                let display_key = format!("{}.{}", module, name);
                if seen.insert(display_key.clone()) {
                    entries.push(Json::obj(vec![
                        ("key", Json::str_val(&display_key)),
                        ("category", Json::str_val("Stdlib")),
                    ]));
                }
            }

            transport::send_response(id, Json::obj(vec![
                ("entries", Json::arr(entries)),
            ]));
            return;
        }

        // Lookup specific doc entry — first try embedded core docs
        if let Some(entry) = crate::lang_docs::get_doc(key) {
            transport::send_response(id, Json::obj(vec![
                ("key", Json::str_val(entry.key)),
                ("category", Json::str_val(&format!("{:?}", entry.category))),
                ("markdown", Json::str_val(entry.markdown)),
            ]));
            return;
        }

        // Then try library docs (loaded from disk)
        if let Some(entry) = self.library_docs.get(key) {
            transport::send_response(id, Json::obj(vec![
                ("key", Json::str_val(&entry.key)),
                ("category", Json::str_val(&entry.category)),
                ("markdown", Json::str_val(&entry.markdown)),
            ]));
            return;
        }

        // Then try stdlib procedure docs (key format: "Module.Proc")
        if let Some(dot_pos) = key.find('.') {
            let module = &key[..dot_pos];
            let name = &key[dot_pos + 1..];
            for (m, n, sig, doc) in crate::stdlib::stdlib_all_proc_docs() {
                if m == module && n == name {
                    let markdown = format!(
                        "# {}.{}\n\n```modula2\n{}\n```\n\n{}",
                        module, name, sig, doc
                    );
                    transport::send_response(id, Json::obj(vec![
                        ("key", Json::str_val(key)),
                        ("category", Json::str_val("Stdlib")),
                        ("markdown", Json::str_val(&markdown)),
                    ]));
                    return;
                }
            }
        }

        transport::send_response(id, Json::Null);
    }

    // ── Helpers ─────────────────────────────────────────────────────

    fn detect_module_name(&mut self, uri: &str) -> Option<String> {
        let result = self.get_analysis(uri)?;
        match result.ast.as_ref()? {
            crate::ast::CompilationUnit::ProgramModule(m) => Some(m.name.clone()),
            crate::ast::CompilationUnit::DefinitionModule(m) => Some(m.name.clone()),
            crate::ast::CompilationUnit::ImplementationModule(m) => Some(m.name.clone()),
        }
    }

    fn analyze_and_publish(&mut self, uri: &str) {
        self.analysis_cache.remove(uri);
        if let Some(result) = self.get_analysis(uri) {
            let diags = diagnostics::errors_to_diagnostics(&result.diagnostics);
            transport::send_notification(
                "textDocument/publishDiagnostics",
                diagnostics::publish_diagnostics(uri, diags),
            );
        }
    }

    fn update_index_for_open_doc(&mut self, uri: &str) {
        if let Some(source) = self.docs.get(uri).map(|s| s.to_string()) {
            if let Some(result) = self.analysis_cache.get(uri).map(|(_, _, r)| r.clone()) {
                let path_str = uri_to_path(uri);
                let path = PathBuf::from(&path_str);
                self.workspace_index.index_from_analysis(&path, uri, &source, result);
            }
        }
    }
}

// ── Free helper functions ───────────────────────────────────────────

fn extract_uri(params: &Json) -> String {
    params.get("textDocument")
        .and_then(|td| td.get("uri"))
        .and_then(|u| u.as_str())
        .unwrap_or("")
        .to_string()
}

fn extract_position(params: &Json) -> (usize, usize) {
    let line = params.get("position")
        .and_then(|p| p.get("line"))
        .and_then(|l| l.as_i64())
        .unwrap_or(0) as usize;
    let col = params.get("position")
        .and_then(|p| p.get("character"))
        .and_then(|c| c.as_i64())
        .unwrap_or(0) as usize;
    (line, col)
}

fn make_location(uri: &str, line: usize, col: usize, len: usize) -> Json {
    Json::obj(vec![
        ("uri", Json::str_val(uri)),
        ("range", Json::obj(vec![
            ("start", Json::obj(vec![
                ("line", Json::int_val((line - 1) as i64)),
                ("character", Json::int_val((col - 1) as i64)),
            ])),
            ("end", Json::obj(vec![
                ("line", Json::int_val((line - 1) as i64)),
                ("character", Json::int_val((col - 1 + len) as i64)),
            ])),
        ])),
    ])
}

fn make_text_edit(line: usize, col: usize, len: usize, new_text: &str) -> Json {
    Json::obj(vec![
        ("range", Json::obj(vec![
            ("start", Json::obj(vec![
                ("line", Json::int_val((line - 1) as i64)),
                ("character", Json::int_val((col - 1) as i64)),
            ])),
            ("end", Json::obj(vec![
                ("line", Json::int_val((line - 1) as i64)),
                ("character", Json::int_val((col - 1 + len) as i64)),
            ])),
        ])),
        ("newText", Json::str_val(new_text)),
    ])
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_uri() {
        let params = Json::obj(vec![
            ("textDocument", Json::obj(vec![
                ("uri", Json::str_val("file:///test.mod")),
            ])),
        ]);
        assert_eq!(extract_uri(&params), "file:///test.mod");
    }

    #[test]
    fn test_extract_position() {
        let params = Json::obj(vec![
            ("position", Json::obj(vec![
                ("line", Json::int_val(5)),
                ("character", Json::int_val(10)),
            ])),
        ]);
        assert_eq!(extract_position(&params), (5, 10));
    }

    #[test]
    fn test_cancellation_tracking() {
        let mut server = LspServer::new(false, Vec::new());
        let id = Json::int_val(42);
        assert!(!server.is_canceled(&id));

        server.canceled_requests.insert(42);
        assert!(server.is_canceled(&id));

        server.retire_cancel(&id);
        assert!(!server.is_canceled(&id));
    }

    #[test]
    fn test_debounce_pending() {
        let mut server = LspServer::new(false, Vec::new());
        server.debounce_ms = 50;

        server.pending_diagnostics.insert("file:///test.mod".to_string(), Instant::now());
        assert_eq!(server.pending_diagnostics.len(), 1);

        std::thread::sleep(Duration::from_millis(60));
        server.flush_pending_diagnostics();
        assert!(server.pending_diagnostics.is_empty());
    }

    #[test]
    fn test_debounce_disabled() {
        let mut server = LspServer::new(false, Vec::new());
        server.debounce_ms = 0;
        server.pending_diagnostics.insert("file:///test.mod".to_string(), Instant::now());
        server.flush_pending_diagnostics();
        assert_eq!(server.pending_diagnostics.len(), 1);
    }

    #[test]
    fn test_multi_root_routing() {
        let mut server = LspServer::new(false, Vec::new());
        server.workspace_roots.push(PathBuf::from("/projects/app1"));
        server.workspace_roots.push(PathBuf::from("/projects/app2"));

        assert_eq!(server.root_for_uri("file:///projects/app1/src/Main.mod"),
                   Some(PathBuf::from("/projects/app1")));
        assert_eq!(server.root_for_uri("file:///projects/app2/src/Main.mod"),
                   Some(PathBuf::from("/projects/app2")));
        assert_eq!(server.root_for_uri("file:///other/file.mod"), None);
    }

    #[test]
    fn test_progress_no_client_support() {
        let mut server = LspServer::new(false, Vec::new());
        server.client_supports_progress = false;
        let token = server.progress_begin("test");
        assert!(token.is_empty());
    }

    #[test]
    fn test_shutdown_rejects_requests() {
        let mut server = LspServer::new(false, Vec::new());
        server.shutdown = true;

        // "exit" should return true (terminate)
        let exit_msg = Json::obj(vec![
            ("jsonrpc", Json::str_val("2.0")),
            ("method", Json::str_val("exit")),
        ]);
        assert!(server.dispatch_message(&exit_msg));

        // Other requests should be rejected (return false, not terminate)
        let hover_msg = Json::obj(vec![
            ("jsonrpc", Json::str_val("2.0")),
            ("id", Json::int_val(1)),
            ("method", Json::str_val("textDocument/hover")),
            ("params", Json::Null),
        ]);
        assert!(!server.dispatch_message(&hover_msg));
    }

    #[test]
    fn test_timer_flush_debounce() {
        // Simulates: didChange records pending, timer tick triggers flush.
        let mut server = LspServer::new(false, Vec::new());
        server.debounce_ms = 30;

        server.pending_diagnostics.insert("file:///test.mod".to_string(), Instant::now());

        // Before debounce window: flush should not clear pending
        server.flush_pending_diagnostics();
        // Immediately after inserting, likely still pending (< 30ms)

        // Wait past debounce
        std::thread::sleep(Duration::from_millis(40));

        // Timer tick would call flush_pending_diagnostics
        server.flush_pending_diagnostics();
        assert!(server.pending_diagnostics.is_empty(), "pending should be flushed after debounce");
    }

    #[test]
    fn test_make_text_edit() {
        let edit = make_text_edit(5, 3, 6, "NewName");
        let range = edit.get("range").unwrap();
        let start = range.get("start").unwrap();
        assert_eq!(start.get("line").and_then(|l| l.as_i64()), Some(4)); // 5-1
        assert_eq!(start.get("character").and_then(|c| c.as_i64()), Some(2)); // 3-1
        let end = range.get("end").unwrap();
        assert_eq!(end.get("character").and_then(|c| c.as_i64()), Some(8)); // 3-1+6
        assert_eq!(edit.get("newText").and_then(|t| t.as_str()), Some("NewName"));
    }
}
