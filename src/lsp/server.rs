use std::path::PathBuf;
use crate::json::Json;
use super::transport;
use super::documents::{DocumentStore, uri_to_path};
use super::analysis::{self, DefCache};
use super::diagnostics;

pub struct LspServer {
    docs: DocumentStore,
    m2plus: bool,
    include_paths: Vec<PathBuf>,
    initialized: bool,
    shutdown: bool,
    def_cache: DefCache,
}

impl LspServer {
    pub fn new(m2plus: bool, include_paths: Vec<PathBuf>) -> Self {
        Self {
            docs: DocumentStore::new(),
            m2plus,
            include_paths,
            initialized: false,
            shutdown: false,
            def_cache: DefCache::new(),
        }
    }

    pub fn run(&mut self) {
        loop {
            let msg = match transport::read_message() {
                Some(m) => m,
                None => break, // EOF
            };

            let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
            let id = msg.get("id");
            let params = msg.get("params");

            match method {
                "initialize" => self.handle_initialize(id.unwrap_or(&Json::Null)),
                "initialized" => { self.initialized = true; }
                "shutdown" => {
                    self.shutdown = true;
                    if let Some(id) = id {
                        transport::send_response(id, Json::Null);
                    }
                }
                "exit" => break,
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
                "textDocument/documentSymbol" => {
                    if let (Some(id), Some(p)) = (id, params) {
                        self.handle_document_symbol(id, p);
                    }
                }
                "textDocument/hover" => {
                    if let (Some(id), Some(p)) = (id, params) {
                        self.handle_hover(id, p);
                    }
                }
                "textDocument/definition" => {
                    if let (Some(id), Some(p)) = (id, params) {
                        self.handle_goto_definition(id, p);
                    }
                }
                "textDocument/completion" => {
                    if let (Some(id), Some(p)) = (id, params) {
                        self.handle_completion(id, p);
                    }
                }
                "textDocument/rename" => {
                    if let (Some(id), Some(p)) = (id, params) {
                        self.handle_rename(id, p);
                    }
                }
                _ => {
                    // Unknown request — send error if it has an id
                    if let Some(id) = id {
                        transport::send_error(id, -32601, &format!("method not found: {}", method));
                    }
                }
            }
        }
    }

    fn handle_initialize(&self, id: &Json) {
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
            ])),
            ("renameProvider", Json::Bool(true)),
        ]);

        let result = Json::obj(vec![
            ("capabilities", capabilities),
            ("serverInfo", Json::obj(vec![
                ("name", Json::str_val("m2c-lsp")),
                ("version", Json::str_val(env!("CARGO_PKG_VERSION"))),
            ])),
        ]);

        transport::send_response(id, result);
    }

    fn handle_did_open(&mut self, params: &Json) {
        if let Some(td) = params.get("textDocument") {
            let uri = td.get("uri").and_then(|u| u.as_str()).unwrap_or("");
            let text = td.get("text").and_then(|t| t.as_str()).unwrap_or("");
            self.docs.open(uri, text.to_string());
            self.analyze_and_publish(uri);
        }
    }

    fn handle_did_change(&mut self, params: &Json) {
        if let Some(td) = params.get("textDocument") {
            let uri = td.get("uri").and_then(|u| u.as_str()).unwrap_or("");
            if let Some(changes) = params.get("contentChanges").and_then(|c| c.as_array()) {
                // Full sync mode — take the last change's text
                if let Some(last) = changes.last() {
                    let text = last.get("text").and_then(|t| t.as_str()).unwrap_or("");
                    self.docs.change(uri, text.to_string());
                    self.analyze_and_publish(uri);
                }
            }
        }
    }

    fn handle_did_close(&mut self, params: &Json) {
        if let Some(td) = params.get("textDocument") {
            let uri = td.get("uri").and_then(|u| u.as_str()).unwrap_or("");
            self.docs.close(uri);
            // Clear diagnostics
            transport::send_notification(
                "textDocument/publishDiagnostics",
                diagnostics::publish_diagnostics(uri, Vec::new()),
            );
        }
    }

    fn handle_did_save(&mut self, params: &Json) {
        if let Some(td) = params.get("textDocument") {
            let uri = td.get("uri").and_then(|u| u.as_str()).unwrap_or("");
            // If text is included, update the document
            if let Some(text) = params.get("text").and_then(|t| t.as_str()) {
                self.docs.change(uri, text.to_string());
            }
            self.analyze_and_publish(uri);
        }
    }

    fn handle_document_symbol(&mut self, id: &Json, params: &Json) {
        let uri = params.get("textDocument")
            .and_then(|td| td.get("uri"))
            .and_then(|u| u.as_str())
            .unwrap_or("");

        if let Some(source) = self.docs.get(uri) {
            let path = uri_to_path(uri);
            let result = analysis::analyze(source, &path, self.m2plus, &self.include_paths, &mut self.def_cache);
            if let Some(ref unit) = result.unit {
                let syms = super::symbols::document_symbols(unit);
                transport::send_response(id, Json::arr(syms));
                return;
            }
        }
        transport::send_response(id, Json::arr(Vec::new()));
    }

    fn handle_hover(&mut self, id: &Json, params: &Json) {
        let uri = params.get("textDocument")
            .and_then(|td| td.get("uri"))
            .and_then(|u| u.as_str())
            .unwrap_or("");
        let line = params.get("position")
            .and_then(|p| p.get("line"))
            .and_then(|l| l.as_i64())
            .unwrap_or(0) as usize;
        let col = params.get("position")
            .and_then(|p| p.get("character"))
            .and_then(|c| c.as_i64())
            .unwrap_or(0) as usize;

        if let Some(source) = self.docs.get(uri) {
            let path = uri_to_path(uri);
            let result = analysis::analyze(source, &path, self.m2plus, &self.include_paths, &mut self.def_cache);
            if let Some(ref symtab) = result.symtab {
                if let Some(hover) = super::hover::hover(source, line, col, symtab) {
                    transport::send_response(id, hover);
                    return;
                }
            }
        }
        transport::send_response(id, Json::Null);
    }

    fn handle_goto_definition(&mut self, id: &Json, params: &Json) {
        let uri = params.get("textDocument")
            .and_then(|td| td.get("uri"))
            .and_then(|u| u.as_str())
            .unwrap_or("");
        let line = params.get("position")
            .and_then(|p| p.get("line"))
            .and_then(|l| l.as_i64())
            .unwrap_or(0) as usize;
        let col = params.get("position")
            .and_then(|p| p.get("character"))
            .and_then(|c| c.as_i64())
            .unwrap_or(0) as usize;

        if let Some(source) = self.docs.get(uri) {
            let path = uri_to_path(uri);
            let result = analysis::analyze(source, &path, self.m2plus, &self.include_paths, &mut self.def_cache);
            if let Some(ref symtab) = result.symtab {
                if let Some(loc) = super::goto_def::goto_definition(
                    source, uri, line, col, symtab, &self.include_paths,
                ) {
                    transport::send_response(id, loc);
                    return;
                }
            }
        }
        transport::send_response(id, Json::Null);
    }

    fn handle_completion(&mut self, id: &Json, params: &Json) {
        let uri = params.get("textDocument")
            .and_then(|td| td.get("uri"))
            .and_then(|u| u.as_str())
            .unwrap_or("");
        let line = params.get("position")
            .and_then(|p| p.get("line"))
            .and_then(|l| l.as_i64())
            .unwrap_or(0) as usize;
        let col = params.get("position")
            .and_then(|p| p.get("character"))
            .and_then(|c| c.as_i64())
            .unwrap_or(0) as usize;

        if let Some(source) = self.docs.get(uri) {
            let path = uri_to_path(uri);
            let result = analysis::analyze(source, &path, self.m2plus, &self.include_paths, &mut self.def_cache);
            if let Some(ref symtab) = result.symtab {
                let completions = super::completion::completion(source, line, col, symtab);
                transport::send_response(id, completions);
                return;
            }
        }
        transport::send_response(id, Json::obj(vec![
            ("isIncomplete", Json::Bool(false)),
            ("items", Json::arr(Vec::new())),
        ]));
    }

    fn handle_rename(&mut self, id: &Json, params: &Json) {
        let uri = params.get("textDocument")
            .and_then(|td| td.get("uri"))
            .and_then(|u| u.as_str())
            .unwrap_or("");
        let line = params.get("position")
            .and_then(|p| p.get("line"))
            .and_then(|l| l.as_i64())
            .unwrap_or(0) as usize;
        let col = params.get("position")
            .and_then(|p| p.get("character"))
            .and_then(|c| c.as_i64())
            .unwrap_or(0) as usize;
        let new_name = params.get("newName")
            .and_then(|n| n.as_str())
            .unwrap_or("");

        if let Some(source) = self.docs.get(uri) {
            let path = uri_to_path(uri);
            let result = analysis::analyze(source, &path, self.m2plus, &self.include_paths, &mut self.def_cache);
            if let Some(ref symtab) = result.symtab {
                if let Some(edit) = super::rename::rename(source, uri, line, col, new_name, symtab) {
                    transport::send_response(id, edit);
                    return;
                }
            }
        }
        transport::send_response(id, Json::Null);
    }

    fn analyze_and_publish(&mut self, uri: &str) {
        if let Some(source) = self.docs.get(uri) {
            let path = uri_to_path(uri);
            let result = analysis::analyze(source, &path, self.m2plus, &self.include_paths, &mut self.def_cache);
            let diags = diagnostics::errors_to_diagnostics(&result.errors);
            transport::send_notification(
                "textDocument/publishDiagnostics",
                diagnostics::publish_diagnostics(uri, diags),
            );
        }
    }
}
