use crate::ast::CompilationUnit;
use crate::json::Json;

/// Build document symbols (outline) from an AST.
pub fn document_symbols(unit: &CompilationUnit) -> Vec<Json> {
    let mut syms = Vec::new();
    match unit {
        CompilationUnit::ProgramModule(m) => {
            add_block_symbols(&m.block, &mut syms);
        }
        CompilationUnit::ImplementationModule(m) => {
            add_block_symbols(&m.block, &mut syms);
        }
        CompilationUnit::DefinitionModule(m) => {
            for def in &m.definitions {
                match def {
                    crate::ast::Definition::Const(c) => {
                        syms.push(make_symbol(&c.name, 14, &c.loc));
                    }
                    crate::ast::Definition::Type(t) => {
                        syms.push(make_symbol(&t.name, 5, &t.loc));
                    }
                    crate::ast::Definition::Var(v) => {
                        for (i, name) in v.names.iter().enumerate() {
                            let loc = v.name_locs.get(i).unwrap_or(&v.loc);
                            syms.push(make_symbol(name, 13, loc));
                        }
                    }
                    crate::ast::Definition::Procedure(p) => {
                        syms.push(make_symbol(&p.name, 12, &p.loc));
                    }
                    _ => {}
                }
            }
        }
    }
    syms
}

fn add_block_symbols(block: &crate::ast::Block, syms: &mut Vec<Json>) {
    for decl in &block.decls {
        match decl {
            crate::ast::Declaration::Const(c) => {
                syms.push(make_symbol(&c.name, 14, &c.loc));
            }
            crate::ast::Declaration::Type(t) => {
                syms.push(make_symbol(&t.name, 5, &t.loc));
            }
            crate::ast::Declaration::Var(v) => {
                for (i, vname) in v.names.iter().enumerate() {
                    let loc = v.name_locs.get(i).unwrap_or(&v.loc);
                    syms.push(make_symbol(vname, 13, loc));
                }
            }
            crate::ast::Declaration::Procedure(p) => {
                syms.push(make_symbol(&p.heading.name, 12, &p.heading.loc));
            }
            _ => {}
        }
    }
}

fn make_symbol(name: &str, kind: i64, loc: &crate::errors::SourceLoc) -> Json {
    let line = if loc.line > 0 { loc.line - 1 } else { 0 };
    let col = if loc.col > 0 { loc.col - 1 } else { 0 };
    let range = Json::obj(vec![
        ("start", Json::obj(vec![
            ("line", Json::int_val(line as i64)),
            ("character", Json::int_val(col as i64)),
        ])),
        ("end", Json::obj(vec![
            ("line", Json::int_val(line as i64)),
            ("character", Json::int_val((col + name.len()) as i64)),
        ])),
    ]);
    Json::obj(vec![
        ("name", Json::str_val(name)),
        ("kind", Json::int_val(kind)),
        ("range", range.clone()),
        ("selectionRange", range),
    ])
}
