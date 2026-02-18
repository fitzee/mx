mod transport;
mod server;
mod documents;
mod analysis;
mod diagnostics;
mod goto_def;
mod completion;
mod hover;
mod rename;
mod symbols;

use std::path::PathBuf;

pub fn run_lsp_server(m2plus: bool, include_paths: Vec<PathBuf>) {
    let mut server = server::LspServer::new(m2plus, include_paths);
    server.run();
}
