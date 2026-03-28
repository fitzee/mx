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
    /// True if this symbol is a VAR parameter (passed by reference).
    pub is_var_param: bool,
    /// True if this symbol is an open array parameter.
    pub is_open_array: bool,
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

    /// Push a scope with an explicit parent (not the current scope).
    /// Used for isolated scopes that should not inherit from the
    /// current scope chain.
    pub fn push_scope_with_parent(&mut self, name: &str, parent: usize) -> usize {
        let id = self.scopes.len();
        self.scopes.push(Scope {
            symbols: HashMap::new(),
            parent: Some(parent),
            name: name.to_string(),
        });
        self.scope_stack.push(id);
        id
    }

    /// Re-enter an existing scope by pushing its ID onto the stack.
    /// Used for two-pass registration where the scope was already created.
    pub fn reenter_scope(&mut self, scope_id: usize) {
        self.scope_stack.push(scope_id);
    }

    pub fn pop_scope(&mut self) -> usize {
        let current = self.current_scope();
        if self.scope_stack.len() > 1 {
            self.scope_stack.pop();
        }
        current
    }

    /// Look up a Type symbol by name, searching ONLY scopes belonging to
    /// the given module (the module scope and its children).
    /// Returns the TypeId if found. Ignores export status.
    pub fn find_type_in_module(&self, module: &str, type_name: &str) -> Option<TypeId> {
        // Search all scopes named after this module for a Type symbol.
        // Scopes are named by enter_scope(module_name) during .def and .mod
        // analysis, so both the .def scope and .mod scope share the name.
        for scope in &self.scopes {
            if scope.name == module {
                if let Some(sym) = scope.symbols.get(type_name) {
                    if matches!(sym.kind, SymbolKind::Type) {
                        return Some(sym.typ);
                    }
                }
            }
        }
        None
    }

    /// Clear all symbols from a scope. Used to clean up temporary scopes
    /// that should not be visible to later lookups.
    pub fn clear_scope(&mut self, scope_id: usize) {
        if scope_id < self.scopes.len() {
            self.scopes[scope_id].symbols.clear();
        }
    }

    pub fn define(&mut self, scope_id: usize, mut sym: Symbol) -> Result<(), String> {
        let scope = &mut self.scopes[scope_id];
        if let Some(existing) = scope.symbols.get(&sym.name) {
            // Local definitions shadow imported names (PIM4 §11)
            if existing.module.is_none() || sym.module.is_some() {
                return Err(format!("'{}' is already defined in this scope", sym.name));
            }
            // Preserve exported flag: if .def exported it, .mod re-declaration
            // must not lose that (unified scope reuse).
            if existing.exported && !sym.exported {
                sym.exported = true;
            }
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
        let mut syms: Vec<&Symbol> = self.scopes[scope_id].symbols.values().collect();
        // Sort by TypeId to approximate source declaration order
        // (lower TypeIds are registered earlier during sequential sema analysis)
        syms.sort_by_key(|s| s.typ);
        syms
    }

    /// Look up a symbol in a specific scope only (no parent chain walk).
    pub fn lookup_in_scope_direct(&self, scope_id: usize, name: &str) -> Option<&Symbol> {
        self.scopes.get(scope_id).and_then(|s| s.symbols.get(name))
    }

    /// Mutable lookup in a specific scope (direct, no parent chain).
    pub fn lookup_in_scope_mut(&mut self, scope_id: usize, name: &str) -> Option<&mut Symbol> {
        self.scopes.get_mut(scope_id).and_then(|s| s.symbols.get_mut(name))
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

    /// Look up a symbol by searching scopes in reverse (innermost first).
    /// This finds the most local binding, avoiding false matches from
    /// outer/global scopes that shadow the intended local.
    pub fn lookup_innermost(&self, name: &str) -> Option<&Symbol> {
        for scope in self.scopes.iter().rev() {
            if let Some(sym) = scope.symbols.get(name) {
                return Some(sym);
            }
        }
        None
    }

    pub fn lookup_qualified(&self, module: &str, name: &str) -> Option<&Symbol> {
        // Find the module's scope by scanning ALL scopes for the Module symbol.
        // This is scope-independent — it does not depend on current_scope.
        let module_scope = self.scopes.iter()
            .find_map(|scope| {
                scope.symbols.get(module)
                    .and_then(|sym| match &sym.kind {
                        SymbolKind::Module { scope_id } => Some(*scope_id),
                        _ => None,
                    })
            });
        if let Some(scope_id) = module_scope {
            let scope = &self.scopes[scope_id];
            if let Some(s) = scope.symbols.get(name) {
                if s.exported {
                    return Some(s);
                }
            }
        }
        None
    }

    pub fn scope_symbols(&self, scope_id: usize) -> impl Iterator<Item = &Symbol> {
        self.scopes[scope_id].symbols.values()
    }

    /// Update a procedure symbol's parameter types and return type in a specific scope.
    /// Used to propagate correctly-resolved types from .mod back to .def scope.
    pub fn update_procedure(&mut self, scope_id: usize, name: &str,
                            params: Vec<ParamInfo>, return_type: Option<TypeId>) {
        if let Some(sym) = self.scopes[scope_id].symbols.get_mut(name) {
            if let SymbolKind::Procedure { params: ref mut p, return_type: ref mut r, .. } = sym.kind {
                *p = params;
                *r = return_type;
            }
        }
    }

    /// Search all scopes for a Type symbol with the given name.
    /// Unlike lookup_any, this skips non-Type symbols (e.g. Module symbols
    /// that share a name with a type they export).
    pub fn find_type(&self, name: &str) -> Option<TypeId> {
        for scope in &self.scopes {
            if let Some(sym) = scope.symbols.get(name) {
                if matches!(sym.kind, SymbolKind::Type) {
                    return Some(sym.typ);
                }
            }
        }
        None
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

    /// Find a type name by its TypeId across all scopes.
    pub fn find_type_by_id(&self, target_id: TypeId) -> Option<String> {
        for scope in &self.scopes {
            for sym in scope.symbols.values() {
                if matches!(sym.kind, SymbolKind::Type) && sym.typ == target_id {
                    return Some(sym.name.clone());
                }
            }
        }
        None
    }

    /// Dump all scopes and their type symbols for diagnostics.
    pub fn dump_type_scopes(&self, target_name: &str) {
        for (i, scope) in self.scopes.iter().enumerate() {
            let types: Vec<&str> = scope.symbols.values()
                .filter(|s| matches!(s.kind, SymbolKind::Type))
                .map(|s| s.name.as_str())
                .collect();
            if types.contains(&target_name) {
                eprintln!("  FOUND '{}' in scope[{}] '{}'", target_name, i, scope.name);
            }
            if !types.is_empty() && types.len() <= 20 {
                eprintln!("  scope[{}] '{}': {:?}", i, scope.name, types);
            } else if !types.is_empty() {
                eprintln!("  scope[{}] '{}': {} types", i, scope.name, types.len());
            }
        }
    }

    /// Return the parent scope ID, if any.
    pub fn scope_parent(&self, scope_id: usize) -> Option<usize> {
        self.scopes.get(scope_id).and_then(|s| s.parent)
    }

    /// Search all scopes for a Module symbol with the given name and return its scope_id.
    /// This bypasses the normal scope chain to handle cases where a type import shadows
    /// the module symbol (e.g., FROM Promise IMPORT Promise where Promise is both module and type).
    pub fn lookup_module_scope(&self, name: &str) -> Option<usize> {
        // First: scan all scopes for a Module symbol with this name.
        for scope in &self.scopes {
            if let Some(sym) = scope.symbols.get(name) {
                if let SymbolKind::Module { scope_id } = &sym.kind {
                    return Some(*scope_id);
                }
            }
        }
        // Fallback: find a scope whose name matches. Prefer the LAST
        // matching scope — .def scopes are created first, .mod scopes
        // (with procedure children) are created later by register_impl_types.
        let mut best = None;
        for (id, scope) in self.scopes.iter().enumerate() {
            if scope.name == name {
                best = Some(id);
            }
        }
        best
    }

    /// Set the exported flag on a symbol in a specific scope.
    pub fn set_exported(&mut self, scope_id: usize, name: &str, exported: bool) {
        if let Some(scope) = self.scopes.get_mut(scope_id) {
            if let Some(sym) = scope.symbols.get_mut(name) {
                sym.exported = exported;
            }
        }
    }

    /// Iterate scopes as (name, symbols) pairs.
    pub fn scopes_iter(&self) -> impl Iterator<Item = (&str, &HashMap<String, Symbol>)> {
        self.scopes.iter().map(|s| (s.name.as_str(), &s.symbols))
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
        // Use lookup_module_scope to find the module, bypassing any non-module
        // symbols with the same name (e.g., a type imported via FROM Module IMPORT TypeName
        // where TypeName == ModuleName).
        if let Some(mod_scope) = self.lookup_module_scope(module) {
            let scope = &self.scopes[mod_scope];
            if let Some(s) = scope.symbols.get(name) {
                if s.exported {
                    return Some((mod_scope, s));
                }
            }
        }
        None
    }
}
