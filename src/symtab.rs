use std::collections::HashMap;
use crate::errors::SourceLoc;
use crate::types::TypeId;

#[derive(Debug, Clone)]
pub struct SymbolTable {
    scopes: Vec<Scope>,
    scope_stack: Vec<usize>,
}

#[derive(Debug, Clone)]
struct Scope {
    symbols: HashMap<String, Symbol>,
    parent: Option<usize>,
    name: String,
}

#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub typ: TypeId,
    pub exported: bool,
    pub module: Option<String>,
    pub loc: SourceLoc,
    pub doc: Option<String>,
}

#[derive(Debug, Clone)]
pub enum SymbolKind {
    Constant(ConstValue),
    Variable,
    Type,
    Procedure {
        params: Vec<ParamInfo>,
        return_type: Option<TypeId>,
        is_builtin: bool,
    },
    Module {
        scope_id: usize,
    },
    Field,
    EnumVariant(i64),
}

#[derive(Debug, Clone)]
pub struct ParamInfo {
    pub name: String,
    pub typ: TypeId,
    pub is_var: bool,
}

#[derive(Debug, Clone)]
pub enum ConstValue {
    Integer(i64),
    Real(f64),
    Boolean(bool),
    Char(char),
    String(String),
    Set(u64),
    Nil,
}

impl SymbolTable {
    pub fn new() -> Self {
        let global = Scope {
            symbols: HashMap::new(),
            parent: None,
            name: "<global>".to_string(),
        };
        Self {
            scopes: vec![global],
            scope_stack: vec![0],
        }
    }

    pub fn current_scope(&self) -> usize {
        *self.scope_stack.last().unwrap()
    }

    pub fn push_scope(&mut self, name: &str) -> usize {
        let parent = self.current_scope();
        let id = self.scopes.len();
        self.scopes.push(Scope {
            symbols: HashMap::new(),
            parent: Some(parent),
            name: name.to_string(),
        });
        self.scope_stack.push(id);
        id
    }

    pub fn pop_scope(&mut self) -> usize {
        let current = self.current_scope();
        if self.scope_stack.len() > 1 {
            self.scope_stack.pop();
        }
        current
    }

    pub fn define(&mut self, scope_id: usize, sym: Symbol) -> Result<(), String> {
        let scope = &mut self.scopes[scope_id];
        if scope.symbols.contains_key(&sym.name) {
            return Err(format!("'{}' is already defined in this scope", sym.name));
        }
        scope.symbols.insert(sym.name.clone(), sym);
        Ok(())
    }

    pub fn define_in_current(&mut self, sym: Symbol) -> Result<(), String> {
        let id = self.current_scope();
        self.define(id, sym)
    }

    pub fn lookup(&self, name: &str) -> Option<&Symbol> {
        self.lookup_in_scope(self.current_scope(), name)
    }

    /// Return all symbols defined directly in the given scope (not inherited from parent).
    pub fn symbols_in_scope(&self, scope_id: usize) -> Vec<&Symbol> {
        if scope_id >= self.scopes.len() { return vec![]; }
        self.scopes[scope_id].symbols.values().collect()
    }

    /// Look up a symbol in a specific scope only (no parent chain walk).
    pub fn lookup_in_scope_direct(&self, scope_id: usize, name: &str) -> Option<&Symbol> {
        self.scopes.get(scope_id).and_then(|s| s.symbols.get(name))
    }

    pub fn lookup_in_scope(&self, scope_id: usize, name: &str) -> Option<&Symbol> {
        let scope = &self.scopes[scope_id];
        if let Some(sym) = scope.symbols.get(name) {
            return Some(sym);
        }
        if let Some(parent) = scope.parent {
            return self.lookup_in_scope(parent, name);
        }
        None
    }

    /// Search ALL scopes for a symbol by name (fallback when current scope is wrong).
    /// Returns the first match found in any scope.
    pub fn lookup_any(&self, name: &str) -> Option<&Symbol> {
        for scope in &self.scopes {
            if let Some(sym) = scope.symbols.get(name) {
                return Some(sym);
            }
        }
        None
    }

    pub fn lookup_qualified(&self, module: &str, name: &str) -> Option<&Symbol> {
        // Find the module symbol, get its scope, look up name there
        if let Some(sym) = self.lookup(module) {
            if let SymbolKind::Module { scope_id } = &sym.kind {
                let scope = &self.scopes[*scope_id];
                if let Some(s) = scope.symbols.get(name) {
                    if s.exported {
                        return Some(s);
                    }
                }
            }
        }
        None
    }

    pub fn scope_symbols(&self, scope_id: usize) -> impl Iterator<Item = &Symbol> {
        self.scopes[scope_id].symbols.values()
    }

    /// Search all scopes for a symbol by name. Returns the first match found.
    /// Useful for LSP features where we need to find a symbol regardless of scope.
    pub fn lookup_all(&self, name: &str) -> Option<&Symbol> {
        for scope in &self.scopes {
            if let Some(sym) = scope.symbols.get(name) {
                return Some(sym);
            }
        }
        None
    }

    /// Return the number of scopes.
    pub fn scope_count(&self) -> usize {
        self.scopes.len()
    }

    /// Return the parent scope ID, if any.
    pub fn scope_parent(&self, scope_id: usize) -> Option<usize> {
        self.scopes.get(scope_id).and_then(|s| s.parent)
    }

    /// Return the name of a scope (e.g. procedure name for a procedure body scope).
    pub fn scope_name(&self, scope_id: usize) -> Option<&str> {
        self.scopes.get(scope_id).map(|s| s.name.as_str())
    }

    /// Look up a symbol by name, returning both the defining scope ID and the symbol.
    /// Walks the parent chain from the current scope.
    pub fn lookup_with_scope(&self, name: &str) -> Option<(usize, &Symbol)> {
        self.lookup_in_scope_with_id(self.current_scope(), name)
    }

    /// Look up a symbol starting from a specific scope, returning (defining_scope_id, &Symbol).
    pub fn lookup_in_scope_with_id(&self, scope_id: usize, name: &str) -> Option<(usize, &Symbol)> {
        let scope = &self.scopes[scope_id];
        if let Some(sym) = scope.symbols.get(name) {
            return Some((scope_id, sym));
        }
        if let Some(parent) = scope.parent {
            return self.lookup_in_scope_with_id(parent, name);
        }
        None
    }

    /// Look up a qualified name, returning (defining_scope_id, &Symbol).
    pub fn lookup_qualified_with_scope(&self, module: &str, name: &str) -> Option<(usize, &Symbol)> {
        if let Some(sym) = self.lookup(module) {
            if let SymbolKind::Module { scope_id } = &sym.kind {
                let scope = &self.scopes[*scope_id];
                if let Some(s) = scope.symbols.get(name) {
                    if s.exported {
                        return Some((*scope_id, s));
                    }
                }
            }
        }
        None
    }
}
