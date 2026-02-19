mod transport;
mod server;
mod documents;
mod analysis;
mod diagnostics;
mod goto_def;
mod completion;
mod hover;
mod lang_docs;
mod rename;
mod references;
mod prepare_rename;
mod symbols;
mod workspace;
mod index;
mod highlight;
mod signature_help;
mod semantic_tokens;
mod code_actions;
mod call_hierarchy;

use std::path::PathBuf;

/// Run the LSP server. Returns exit code: 0 if shutdown received, 1 otherwise.
pub fn run_lsp_server(m2plus: bool, include_paths: Vec<PathBuf>) -> i32 {
    let mut server = server::LspServer::new(m2plus, include_paths);
    server.run()
}
