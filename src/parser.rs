use std::collections::HashMap;
use crate::ast::*;
use crate::errors::{CompileError, CompileResult, SourceLoc};
use crate::token::{Token, TokenKind};

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    errors: Vec<CompileError>,
    panic_mode: bool,
    /// Maps token position → doc comment text (from preceding (** or (*! comment)
    doc_map: HashMap<usize, String>,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        // Filter out DocComment tokens, building a map from the following
        // token's position to the doc comment text.
        let mut filtered = Vec::new();
        let mut doc_map: HashMap<usize, String> = HashMap::new();
        let mut pending_doc: Option<String> = None;

        for tok in tokens {
            if let TokenKind::DocComment(text) = tok.kind {
                pending_doc = Some(text);
            } else {
                if let Some(doc) = pending_doc.take() {
                    doc_map.insert(filtered.len(), doc);
                }
                filtered.push(tok);
            }
        }

        Self {
            tokens: filtered,
            pos: 0,
            errors: Vec::new(),
            panic_mode: false,
            doc_map,
        }
    }

    /// Take the doc comment associated with the current token position, if any.
    fn take_doc(&mut self) -> Option<String> {
        self.doc_map.remove(&self.pos)
    }

    // ── Helpers ─────────────────────────────────────────────────────

    fn peek(&self) -> &TokenKind {
        &self.tokens[self.pos].kind
    }

    fn loc(&self) -> SourceLoc {
        self.tokens[self.pos].loc.clone()
    }

    fn advance(&mut self) -> &Token {
        let tok = &self.tokens[self.pos];
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
        tok
    }

    fn expect(&mut self, expected: &TokenKind) -> CompileResult<SourceLoc> {
        let loc = self.loc();
        if std::mem::discriminant(self.peek()) == std::mem::discriminant(expected) {
            self.advance();
            Ok(loc)
        } else {
            Err(CompileError::parser(
                loc,
                format!("expected {}, found {}", expected.describe(), self.peek().describe()),
            ))
        }
    }

    fn expect_ident(&mut self) -> CompileResult<Ident> {
        let loc = self.loc();
        match self.peek().clone() {
            TokenKind::Ident(name) => {
                self.advance();
                Ok(name)
            }
            _ => Err(CompileError::parser(
                loc,
                format!("expected identifier, found {}", self.peek().describe()),
            )),
        }
    }

    fn record_error(&mut self, err: CompileError) {
        if !self.panic_mode {
            self.errors.push(err);
        }
        self.panic_mode = true;
    }

    /// Synchronize after a parse error - skip tokens until we find a synchronization point
    fn synchronize(&mut self) {
        self.panic_mode = false;
        while !self.at(&TokenKind::Eof) {
            match self.peek() {
                // Statement-starting keywords
                TokenKind::If | TokenKind::While | TokenKind::Repeat
                | TokenKind::For | TokenKind::Loop | TokenKind::With
                | TokenKind::Case | TokenKind::Return | TokenKind::Exit
                | TokenKind::Try | TokenKind::Lock | TokenKind::Typecase => return,
                // Declaration-starting keywords
                TokenKind::Const | TokenKind::Type | TokenKind::Var
                | TokenKind::Procedure | TokenKind::Module => return,
                // Block delimiters
                TokenKind::Begin | TokenKind::End => return,
                // After a semicolon, we're likely at a new statement/declaration
                TokenKind::Semi => {
                    self.advance();
                    return;
                }
                _ => {
                    self.advance();
                }
            }
        }
    }

    /// Return all accumulated errors
    pub fn get_errors(&self) -> &[CompileError] {
        &self.errors
    }

    fn eat(&mut self, kind: &TokenKind) -> bool {
        if std::mem::discriminant(self.peek()) == std::mem::discriminant(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn at(&self, kind: &TokenKind) -> bool {
        std::mem::discriminant(self.peek()) == std::mem::discriminant(kind)
    }

    // ── Top-level ───────────────────────────────────────────────────

    pub fn parse_compilation_unit(&mut self) -> CompileResult<CompilationUnit> {
        let doc = self.take_doc();
        let result = match self.peek() {
            TokenKind::Definition => {
                self.parse_definition_module().map(|mut m| { m.doc = m.doc.or(doc.clone()); CompilationUnit::DefinitionModule(m) })
            }
            TokenKind::Implementation => {
                self.parse_implementation_module().map(|mut m| { m.doc = m.doc.or(doc.clone()); CompilationUnit::ImplementationModule(m) })
            }
            TokenKind::Module => {
                self.parse_program_module().map(|mut m| { m.doc = m.doc.or(doc.clone()); CompilationUnit::ProgramModule(m) })
            }
            TokenKind::Safe | TokenKind::Unsafe => {
                // SAFE/UNSAFE prefix — peek ahead to determine module kind
                let is_safe = self.at(&TokenKind::Safe);
                let is_unsafe = self.at(&TokenKind::Unsafe);
                self.advance(); // consume SAFE/UNSAFE
                let doc2 = self.take_doc().or(doc.clone());
                match self.peek() {
                    TokenKind::Module => {
                        let mut m = self.parse_program_module()?;
                        m.is_safe = is_safe;
                        m.is_unsafe = is_unsafe;
                        m.doc = m.doc.or(doc2);
                        Ok(CompilationUnit::ProgramModule(m))
                    }
                    TokenKind::Definition => {
                        let mut m = self.parse_definition_module()?;
                        m.is_safe = is_safe;
                        m.is_unsafe = is_unsafe;
                        m.doc = m.doc.or(doc2);
                        Ok(CompilationUnit::DefinitionModule(m))
                    }
                    TokenKind::Implementation => {
                        let mut m = self.parse_implementation_module()?;
                        m.is_safe = is_safe;
                        m.is_unsafe = is_unsafe;
                        m.doc = m.doc.or(doc2);
                        Ok(CompilationUnit::ImplementationModule(m))
                    }
                    _ => Err(CompileError::parser(
                        self.loc(),
                        "expected MODULE, DEFINITION, or IMPLEMENTATION after SAFE/UNSAFE".to_string(),
                    )),
                }
            }
            _ => Err(CompileError::parser(
                self.loc(),
                "expected MODULE, DEFINITION, or IMPLEMENTATION".to_string(),
            )),
        };

        // If we have accumulated errors from recovery, report them
        if !self.errors.is_empty() {
            let msg = self.errors
                .iter()
                .map(|e| format!("{}", e))
                .collect::<Vec<_>>()
                .join("\n");
            return Err(CompileError::parser(
                self.errors[0].loc.clone(),
                msg,
            ));
        }

        result
    }

    fn parse_program_module(&mut self) -> CompileResult<ProgramModule> {
        let loc = self.loc();
        self.expect(&TokenKind::Module)?;
        let name = self.expect_ident()?;
        let priority = if self.eat(&TokenKind::LBrack) {
            let e = self.parse_expression()?;
            self.expect(&TokenKind::RBrack)?;
            Some(Box::new(e))
        } else {
            None
        };
        self.expect(&TokenKind::Semi)?;
        let imports = self.parse_imports()?;
        let block = self.parse_block()?;
        let end_name = self.expect_ident()?;
        if end_name != name {
            return Err(CompileError::parser(
                self.loc(),
                format!("module name mismatch: expected '{}', found '{}'", name, end_name),
            ));
        }
        self.expect(&TokenKind::Dot)?;
        Ok(ProgramModule { name, priority, imports, export: None, block, is_safe: false, is_unsafe: false, loc, doc: None })
    }

    /// Parse a local (nested) module inside a block.
    /// Syntax: MODULE name ; [IMPORT ...;] [EXPORT [QUALIFIED] name {, name} ;] block END name ;
    fn parse_local_module(&mut self) -> CompileResult<ProgramModule> {
        let loc = self.loc();
        self.expect(&TokenKind::Module)?;
        let name = self.expect_ident()?;
        let priority = if self.eat(&TokenKind::LBrack) {
            let e = self.parse_expression()?;
            self.expect(&TokenKind::RBrack)?;
            Some(Box::new(e))
        } else {
            None
        };
        self.expect(&TokenKind::Semi)?;
        let imports = self.parse_imports()?;
        let export = if self.at(&TokenKind::Export) {
            Some(self.parse_export()?)
        } else {
            None
        };
        let block = self.parse_block()?;
        let end_name = self.expect_ident()?;
        if end_name != name {
            return Err(CompileError::parser(
                self.loc(),
                format!("module name mismatch: expected '{}', found '{}'", name, end_name),
            ));
        }
        self.expect(&TokenKind::Semi)?;
        Ok(ProgramModule { name, priority, imports, export, block, is_safe: false, is_unsafe: false, loc, doc: None })
    }

    fn parse_definition_module(&mut self) -> CompileResult<DefinitionModule> {
        let loc = self.loc();
        self.expect(&TokenKind::Definition)?;
        self.expect(&TokenKind::Module)?;
        let foreign_lang = if self.at(&TokenKind::For) {
            self.advance();
            if let TokenKind::StringLit(lang) = self.peek().clone() {
                self.advance();
                Some(lang)
            } else {
                return Err(CompileError::parser(
                    self.loc(),
                    "expected string literal after FOR (e.g., \"C\")".to_string(),
                ));
            }
        } else {
            None
        };
        let name = self.expect_ident()?;
        self.expect(&TokenKind::Semi)?;
        let imports = self.parse_imports()?;
        let export = if self.at(&TokenKind::Export) {
            Some(self.parse_export()?)
        } else {
            None
        };
        let mut definitions = Vec::new();
        loop {
            let section_doc = self.take_doc();
            match self.peek() {
                TokenKind::Const => {
                    self.advance();
                    let mut first = true;
                    while let TokenKind::Ident(_) = self.peek() {
                        let doc = self.take_doc().or_else(|| if first { section_doc.clone() } else { None });
                        first = false;
                        let mut c = self.parse_const_decl()?;
                        c.doc = doc;
                        definitions.push(Definition::Const(c));
                    }
                }
                TokenKind::Type => {
                    self.advance();
                    let mut first = true;
                    while let TokenKind::Ident(_) = self.peek() {
                        let doc = self.take_doc().or_else(|| if first { section_doc.clone() } else { None });
                        first = false;
                        let mut t = self.parse_type_decl_def()?;
                        t.doc = doc;
                        definitions.push(Definition::Type(t));
                    }
                }
                TokenKind::Var => {
                    self.advance();
                    let mut first = true;
                    while let TokenKind::Ident(_) = self.peek() {
                        let doc = self.take_doc().or_else(|| if first { section_doc.clone() } else { None });
                        first = false;
                        let mut v = self.parse_var_decl()?;
                        v.doc = doc;
                        definitions.push(Definition::Var(v));
                    }
                }
                TokenKind::Pragma(_, _) | TokenKind::Procedure => {
                    let doc = section_doc.or_else(|| self.take_doc());
                    let ec_name = self.try_consume_exportc_pragma();
                    let doc = doc.or_else(|| self.take_doc());
                    let mut heading = self.parse_proc_heading()?;
                    heading.export_c_name = ec_name;
                    heading.doc = doc;
                    self.expect(&TokenKind::Semi)?;
                    definitions.push(Definition::Procedure(heading));
                }
                TokenKind::Exception => {
                    let doc = section_doc;
                    self.advance();
                    let eloc = self.loc();
                    let ename = self.expect_ident()?;
                    self.expect(&TokenKind::Semi)?;
                    definitions.push(Definition::Exception(ExceptionDecl { name: ename, loc: eloc, doc }));
                }
                _ => break,
            }
        }
        self.expect(&TokenKind::End)?;
        let end_name = self.expect_ident()?;
        if end_name != name {
            return Err(CompileError::parser(
                self.loc(),
                format!("module name mismatch: expected '{}', found '{}'", name, end_name),
            ));
        }
        self.expect(&TokenKind::Dot)?;
        Ok(DefinitionModule { name, imports, export, definitions, is_safe: false, is_unsafe: false, foreign_lang, loc, doc: None })
    }

    fn parse_implementation_module(&mut self) -> CompileResult<ImplementationModule> {
        let loc = self.loc();
        self.expect(&TokenKind::Implementation)?;
        self.expect(&TokenKind::Module)?;
        let name = self.expect_ident()?;
        let priority = if self.eat(&TokenKind::LBrack) {
            let e = self.parse_expression()?;
            self.expect(&TokenKind::RBrack)?;
            Some(Box::new(e))
        } else {
            None
        };
        self.expect(&TokenKind::Semi)?;
        let imports = self.parse_imports()?;
        let block = self.parse_block()?;
        let end_name = self.expect_ident()?;
        if end_name != name {
            return Err(CompileError::parser(
                self.loc(),
                format!("module name mismatch: expected '{}', found '{}'", name, end_name),
            ));
        }
        self.expect(&TokenKind::Dot)?;
        Ok(ImplementationModule { name, priority, imports, block, is_safe: false, is_unsafe: false, loc, doc: None })
    }

    // ── Imports / Exports ───────────────────────────────────────────

    fn parse_imports(&mut self) -> CompileResult<Vec<Import>> {
        let mut imports = Vec::new();
        loop {
            match self.peek() {
                TokenKind::Import | TokenKind::From => {
                    imports.push(self.parse_import()?);
                }
                _ => break,
            }
        }
        Ok(imports)
    }

    fn parse_import(&mut self) -> CompileResult<Import> {
        let loc = self.loc();
        let from_module = if self.eat(&TokenKind::From) {
            let m = self.expect_ident()?;
            Some(m)
        } else {
            None
        };
        self.expect(&TokenKind::Import)?;
        let mut names = Vec::new();
        loop {
            let name = self.expect_ident()?;
            let alias = if from_module.is_some() && self.eat(&TokenKind::As) {
                Some(self.expect_ident()?)
            } else {
                None
            };
            names.push(ImportName { name, alias });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(&TokenKind::Semi)?;
        Ok(Import { from_module, names, loc })
    }

    fn parse_export(&mut self) -> CompileResult<Export> {
        let loc = self.loc();
        self.expect(&TokenKind::Export)?;
        let qualified = self.eat(&TokenKind::Qualified);
        let mut names = vec![self.expect_ident()?];
        while self.eat(&TokenKind::Comma) {
            names.push(self.expect_ident()?);
        }
        self.expect(&TokenKind::Semi)?;
        Ok(Export { qualified, names, loc })
    }

    // ── Block & declarations ────────────────────────────────────────

    fn parse_block(&mut self) -> CompileResult<Block> {
        let loc = self.loc();
        let decls = self.parse_declarations()?;
        let body = if self.eat(&TokenKind::Begin) {
            Some(self.parse_statement_sequence()?)
        } else {
            None
        };
        // ISO Modula-2: EXCEPT clause (exception handler)
        let except = if self.eat(&TokenKind::Except) {
            Some(self.parse_statement_sequence()?)
        } else {
            None
        };
        // ISO Modula-2: FINALLY clause (module termination)
        let finally = if self.eat(&TokenKind::Finally) {
            Some(self.parse_statement_sequence()?)
        } else {
            None
        };
        self.expect(&TokenKind::End)?;
        Ok(Block { decls, body, finally, except, loc })
    }

    fn parse_declarations(&mut self) -> CompileResult<Vec<Declaration>> {
        let mut decls = Vec::new();
        loop {
            let section_doc = self.take_doc();
            match self.peek() {
                TokenKind::Const => {
                    self.advance();
                    let mut first = true;
                    while let TokenKind::Ident(_) = self.peek() {
                        let doc = self.take_doc().or_else(|| if first { section_doc.clone() } else { None });
                        first = false;
                        match self.parse_const_decl() {
                            Ok(mut c) => { c.doc = doc; decls.push(Declaration::Const(c)); }
                            Err(e) => {
                                self.record_error(e);
                                self.synchronize();
                            }
                        }
                    }
                }
                TokenKind::Type => {
                    self.advance();
                    let mut first = true;
                    while let TokenKind::Ident(_) = self.peek() {
                        let doc = self.take_doc().or_else(|| if first { section_doc.clone() } else { None });
                        first = false;
                        match self.parse_type_decl() {
                            Ok(mut t) => { t.doc = doc; decls.push(Declaration::Type(t)); }
                            Err(e) => {
                                self.record_error(e);
                                self.synchronize();
                            }
                        }
                    }
                }
                TokenKind::Var => {
                    self.advance();
                    let mut first = true;
                    while let TokenKind::Ident(_) = self.peek() {
                        let doc = self.take_doc().or_else(|| if first { section_doc.clone() } else { None });
                        first = false;
                        match self.parse_var_decl() {
                            Ok(mut v) => { v.doc = doc; decls.push(Declaration::Var(v)); }
                            Err(e) => {
                                self.record_error(e);
                                self.synchronize();
                            }
                        }
                    }
                }
                TokenKind::Pragma(_, _) | TokenKind::Procedure => {
                    let doc = section_doc.or_else(|| self.take_doc());
                    let ec_name = self.try_consume_exportc_pragma();
                    let doc = doc.or_else(|| self.take_doc());
                    match self.parse_proc_decl() {
                        Ok(mut p) => {
                            p.heading.export_c_name = ec_name;
                            p.doc = doc;
                            decls.push(Declaration::Procedure(p));
                        }
                        Err(e) => {
                            self.record_error(e);
                            self.synchronize();
                        }
                    }
                }
                TokenKind::Module => {
                    let doc = section_doc;
                    match self.parse_local_module() {
                        Ok(mut m) => { m.doc = doc; decls.push(Declaration::Module(m)); }
                        Err(e) => {
                            self.record_error(e);
                            self.synchronize();
                        }
                    }
                }
                // Modula-2+ EXCEPTION declaration
                TokenKind::Exception => {
                    let doc = section_doc;
                    self.advance();
                    let eloc = self.loc();
                    let ename = self.expect_ident()?;
                    self.expect(&TokenKind::Semi)?;
                    decls.push(Declaration::Exception(ExceptionDecl { name: ename, loc: eloc, doc }));
                }
                _ => break,
            }
        }
        Ok(decls)
    }

    fn parse_const_decl(&mut self) -> CompileResult<ConstDecl> {
        let loc = self.loc();
        let name = self.expect_ident()?;
        self.expect(&TokenKind::Eq)?;
        let expr = self.parse_expression()?;
        self.expect(&TokenKind::Semi)?;
        Ok(ConstDecl { name, expr, loc, doc: None })
    }

    fn parse_type_decl(&mut self) -> CompileResult<TypeDecl> {
        let loc = self.loc();
        let name = self.expect_ident()?;
        self.expect(&TokenKind::Eq)?;
        let typ = Some(self.parse_type()?);
        self.expect(&TokenKind::Semi)?;
        Ok(TypeDecl { name, typ, loc, doc: None })
    }

    fn parse_type_decl_def(&mut self) -> CompileResult<TypeDecl> {
        let loc = self.loc();
        let name = self.expect_ident()?;
        // In definition modules, type can be opaque (just name;)
        let typ = if self.eat(&TokenKind::Eq) {
            let t = self.parse_type()?;
            Some(t)
        } else {
            None
        };
        self.expect(&TokenKind::Semi)?;
        Ok(TypeDecl { name, typ, loc, doc: None })
    }

    fn parse_var_decl(&mut self) -> CompileResult<VarDecl> {
        let loc = self.loc();
        let mut name_locs = vec![loc.clone()];
        let mut names = vec![self.expect_ident()?];
        while self.eat(&TokenKind::Comma) {
            name_locs.push(self.loc());
            names.push(self.expect_ident()?);
        }
        self.expect(&TokenKind::Colon)?;
        let typ = self.parse_type()?;
        self.expect(&TokenKind::Semi)?;
        Ok(VarDecl { names, name_locs, typ, loc, doc: None })
    }

    fn parse_proc_decl(&mut self) -> CompileResult<ProcDecl> {
        let loc = self.loc();
        let heading = self.parse_proc_heading()?;
        self.expect(&TokenKind::Semi)?;
        let block = self.parse_block()?;
        let end_name = self.expect_ident()?;
        if end_name != heading.name {
            return Err(CompileError::parser(
                self.loc(),
                format!(
                    "procedure name mismatch: expected '{}', found '{}'",
                    heading.name, end_name
                ),
            ));
        }
        self.expect(&TokenKind::Semi)?;
        Ok(ProcDecl { heading, block, loc, doc: None })
    }

    fn try_consume_exportc_pragma(&mut self) -> Option<String> {
        if let TokenKind::Pragma(ref dir, ref arg) = self.peek().clone() {
            if dir == "EXPORTC" {
                let name = arg.clone();
                self.advance();
                return name;
            }
        }
        None
    }

    fn parse_proc_heading(&mut self) -> CompileResult<ProcHeading> {
        let loc = self.loc();
        self.expect(&TokenKind::Procedure)?;
        let name = self.expect_ident()?;
        let mut params = Vec::new();
        let mut return_type = None;
        if self.eat(&TokenKind::LParen) {
            if !self.at(&TokenKind::RParen) {
                params = self.parse_formal_params()?;
            }
            self.expect(&TokenKind::RParen)?;
            if self.eat(&TokenKind::Colon) {
                return_type = Some(Box::new(self.parse_qual_type()?));
            }
        }
        // Modula-2+ RAISES clause
        let raises = if self.at(&TokenKind::Ident(String::new())) {
            if let TokenKind::Ident(ref s) = self.peek().clone() {
                if s == "RAISES" {
                    self.advance(); // consume RAISES
                    self.expect(&TokenKind::LBrace)?;
                    let mut exceptions = Vec::new();
                    if !self.at(&TokenKind::RBrace) {
                        exceptions.push(self.parse_qualident()?);
                        while self.eat(&TokenKind::Comma) {
                            exceptions.push(self.parse_qualident()?);
                        }
                    }
                    self.expect(&TokenKind::RBrace)?;
                    Some(exceptions)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };
        Ok(ProcHeading { name, params, return_type, raises, export_c_name: None, loc, doc: None })
    }

    fn parse_formal_params(&mut self) -> CompileResult<Vec<FormalParam>> {
        let mut params = vec![self.parse_formal_param()?];
        while self.eat(&TokenKind::Semi) {
            params.push(self.parse_formal_param()?);
        }
        Ok(params)
    }

    fn parse_formal_param(&mut self) -> CompileResult<FormalParam> {
        let loc = self.loc();
        let is_var = self.eat(&TokenKind::Var);
        let mut names = vec![self.expect_ident()?];
        while self.eat(&TokenKind::Comma) {
            names.push(self.expect_ident()?);
        }
        self.expect(&TokenKind::Colon)?;
        let typ = self.parse_formal_type()?;
        Ok(FormalParam { is_var, names, typ, loc })
    }

    fn parse_formal_type(&mut self) -> CompileResult<TypeNode> {
        if self.eat(&TokenKind::Array) {
            self.expect(&TokenKind::Of)?;
            let elem = self.parse_formal_type()?;
            Ok(TypeNode::OpenArray {
                elem_type: Box::new(elem),
                loc: self.loc(),
            })
        } else if self.at(&TokenKind::Refany) {
            let loc = self.loc();
            self.advance();
            Ok(TypeNode::RefAny { loc })
        } else {
            self.parse_qual_type()
        }
    }

    fn parse_qual_type(&mut self) -> CompileResult<TypeNode> {
        let qi = self.parse_qualident()?;
        Ok(TypeNode::Named(qi))
    }

    // ── Types ───────────────────────────────────────────────────────

    fn parse_type(&mut self) -> CompileResult<TypeNode> {
        match self.peek() {
            TokenKind::Array => self.parse_array_type(),
            TokenKind::Record => self.parse_record_type(),
            TokenKind::Set => self.parse_set_type(),
            TokenKind::Pointer => self.parse_pointer_type(),
            TokenKind::Procedure => self.parse_procedure_type(),
            TokenKind::LParen => self.parse_enumeration_type(),
            TokenKind::LBrack => self.parse_subrange_type(),
            // Modula-2+ REF type
            TokenKind::Ref => self.parse_ref_type(),
            // Modula-2+ BRANDED REF type
            TokenKind::Branded => self.parse_branded_ref_type(),
            // Modula-2+ REFANY built-in type
            TokenKind::Refany => {
                let loc = self.loc();
                self.advance();
                Ok(TypeNode::RefAny { loc })
            }
            // Modula-2+ OBJECT type
            TokenKind::Object => self.parse_object_type(None),
            TokenKind::Ident(_) => {
                let qi = self.parse_qualident()?;
                // Could be subrange: Ident .. Expr
                if self.at(&TokenKind::DotDot) {
                    // qi must be a simple ident turned into an expr
                    let loc = qi.loc.clone();
                    let low = Expr {
                        kind: ExprKind::Designator(Designator {
                            ident: qi,
                            selectors: vec![],
                            loc: loc.clone(),
                        }),
                        loc: loc.clone(),
                    };
                    self.expect(&TokenKind::DotDot)?;
                    let high = self.parse_expression()?;
                    Ok(TypeNode::Subrange {
                        low: Box::new(low),
                        high: Box::new(high),
                        loc,
                    })
                } else if self.at(&TokenKind::Object) {
                    // Parent OBJECT ... END (M2+ inheritance)
                    self.parse_object_type(Some(qi))
                } else {
                    Ok(TypeNode::Named(qi))
                }
            }
            TokenKind::IntLit(_) | TokenKind::CharLit(_) => {
                self.parse_subrange_with_expr()
            }
            _ => Err(CompileError::parser(self.loc(), "expected type".to_string())),
        }
    }

    fn parse_subrange_with_expr(&mut self) -> CompileResult<TypeNode> {
        let loc = self.loc();
        let low = self.parse_expression()?;
        self.expect(&TokenKind::DotDot)?;
        let high = self.parse_expression()?;
        Ok(TypeNode::Subrange {
            low: Box::new(low),
            high: Box::new(high),
            loc,
        })
    }

    fn parse_subrange_type(&mut self) -> CompileResult<TypeNode> {
        let loc = self.loc();
        self.expect(&TokenKind::LBrack)?;
        let low = self.parse_expression()?;
        self.expect(&TokenKind::DotDot)?;
        let high = self.parse_expression()?;
        self.expect(&TokenKind::RBrack)?;
        Ok(TypeNode::Subrange {
            low: Box::new(low),
            high: Box::new(high),
            loc,
        })
    }

    fn parse_array_type(&mut self) -> CompileResult<TypeNode> {
        let loc = self.loc();
        self.expect(&TokenKind::Array)?;
        let mut index_types = vec![self.parse_type()?];
        while self.eat(&TokenKind::Comma) {
            index_types.push(self.parse_type()?);
        }
        self.expect(&TokenKind::Of)?;
        let elem_type = self.parse_type()?;
        Ok(TypeNode::Array {
            index_types,
            elem_type: Box::new(elem_type),
            loc,
        })
    }

    fn parse_record_type(&mut self) -> CompileResult<TypeNode> {
        let loc = self.loc();
        self.expect(&TokenKind::Record)?;
        let fields = self.parse_field_lists()?;
        self.expect(&TokenKind::End)?;
        Ok(TypeNode::Record { fields, loc })
    }

    fn parse_field_lists(&mut self) -> CompileResult<Vec<FieldList>> {
        let mut lists = Vec::new();
        // Parse first field list
        if !self.at(&TokenKind::End) {
            lists.push(self.parse_field_list()?);
            while self.eat(&TokenKind::Semi) {
                if self.at(&TokenKind::End) {
                    break;
                }
                lists.push(self.parse_field_list()?);
            }
        }
        Ok(lists)
    }

    fn parse_field_list(&mut self) -> CompileResult<FieldList> {
        let mut fixed = Vec::new();
        let mut variant = None;

        if self.at(&TokenKind::Case) {
            variant = Some(self.parse_variant_part()?);
        } else if let TokenKind::Ident(_) = self.peek() {
            // fixed fields
            loop {
                let loc = self.loc();
                let mut names = vec![self.expect_ident()?];
                while self.eat(&TokenKind::Comma) {
                    names.push(self.expect_ident()?);
                }
                self.expect(&TokenKind::Colon)?;
                let typ = self.parse_type()?;
                fixed.push(Field { names, typ, loc });
                // Check if next is another field (ident followed by , or :)
                // or variant or end of record
                if self.at(&TokenKind::Semi) {
                    // peek past semi to see if it's more fixed fields or variant
                    break;
                } else {
                    break;
                }
            }
        }
        Ok(FieldList { fixed, variant })
    }

    fn parse_variant_part(&mut self) -> CompileResult<VariantPart> {
        let loc = self.loc();
        self.expect(&TokenKind::Case)?;

        // CASE [tag_name] : tag_type OF
        let first_ident = self.expect_ident()?;
        let (tag_name, tag_type) = if self.eat(&TokenKind::Colon) {
            let tt = self.parse_qualident()?;
            (Some(first_ident), tt)
        } else {
            // no tag variable, first_ident IS the type
            let qi = QualIdent {
                module: None,
                name: first_ident,
                loc: loc.clone(),
            };
            (None, qi)
        };

        self.expect(&TokenKind::Of)?;
        let mut variants = Vec::new();
        loop {
            if self.at(&TokenKind::End) || self.at(&TokenKind::Else) {
                break;
            }
            variants.push(self.parse_variant()?);
            if !self.eat(&TokenKind::Pipe) {
                break;
            }
        }
        self.expect(&TokenKind::End)?;
        Ok(VariantPart { tag_name, tag_type, variants, loc })
    }

    fn parse_variant(&mut self) -> CompileResult<Variant> {
        let loc = self.loc();
        let labels = self.parse_case_labels()?;
        self.expect(&TokenKind::Colon)?;
        let fields = self.parse_field_lists()?;
        Ok(Variant { labels, fields, loc })
    }

    fn parse_set_type(&mut self) -> CompileResult<TypeNode> {
        let loc = self.loc();
        self.expect(&TokenKind::Set)?;
        self.expect(&TokenKind::Of)?;
        let base = self.parse_type()?;
        Ok(TypeNode::Set { base: Box::new(base), loc })
    }

    fn parse_pointer_type(&mut self) -> CompileResult<TypeNode> {
        let loc = self.loc();
        self.expect(&TokenKind::Pointer)?;
        self.expect(&TokenKind::To)?;
        let base = self.parse_type()?;
        Ok(TypeNode::Pointer { base: Box::new(base), loc })
    }

    /// Parse REF T
    fn parse_ref_type(&mut self) -> CompileResult<TypeNode> {
        let loc = self.loc();
        self.expect(&TokenKind::Ref)?;
        let target = self.parse_type()?;
        Ok(TypeNode::Ref { target: Box::new(target), branded: None, loc })
    }

    /// Parse BRANDED "tag" REF T
    fn parse_branded_ref_type(&mut self) -> CompileResult<TypeNode> {
        let loc = self.loc();
        self.expect(&TokenKind::Branded)?;
        let brand = if let TokenKind::StringLit(s) = self.peek().clone() {
            self.advance();
            Some(s)
        } else {
            None // unbranded — BRANDED REF T (unique anonymous brand)
        };
        self.expect(&TokenKind::Ref)?;
        let target = self.parse_type()?;
        Ok(TypeNode::Ref { target: Box::new(target), branded: brand, loc })
    }

    /// Parse OBJECT type: [parent] OBJECT fields [METHODS methods] [OVERRIDES overrides] END
    fn parse_object_type(&mut self, parent: Option<QualIdent>) -> CompileResult<TypeNode> {
        let loc = self.loc();
        self.expect(&TokenKind::Object)?;
        let mut fields = Vec::new();
        let mut methods = Vec::new();
        let mut overrides = Vec::new();

        // Parse fields until METHODS, OVERRIDES, or END
        while !self.at(&TokenKind::Methods) && !self.at(&TokenKind::Override)
            && !self.at(&TokenKind::End) && !self.at(&TokenKind::Eof)
        {
            if let TokenKind::Ident(_) = self.peek() {
                let field_loc = self.loc();
                let mut names = vec![self.expect_ident()?];
                while self.eat(&TokenKind::Comma) {
                    names.push(self.expect_ident()?);
                }
                self.expect(&TokenKind::Colon)?;
                let typ = self.parse_type()?;
                self.expect(&TokenKind::Semi)?;
                fields.push(Field { names, typ, loc: field_loc });
            } else {
                break;
            }
        }

        // Parse METHODS section
        if self.eat(&TokenKind::Methods) {
            while let TokenKind::Ident(_) = self.peek() {
                let mloc = self.loc();
                let name = self.expect_ident()?;
                let mut params = Vec::new();
                let mut return_type = None;
                if self.eat(&TokenKind::LParen) {
                    if !self.at(&TokenKind::RParen) {
                        params = self.parse_formal_params()?;
                    }
                    self.expect(&TokenKind::RParen)?;
                    if self.eat(&TokenKind::Colon) {
                        return_type = Some(Box::new(self.parse_qual_type()?));
                    }
                }
                self.expect(&TokenKind::Semi)?;
                methods.push(MethodDecl { name, params, return_type, loc: mloc });
            }
        }

        // Parse OVERRIDES section
        if self.eat(&TokenKind::Override) {
            while let TokenKind::Ident(_) = self.peek() {
                let oloc = self.loc();
                let name = self.expect_ident()?;
                self.expect(&TokenKind::Assign)?;
                let proc_name = self.parse_qualident()?;
                self.expect(&TokenKind::Semi)?;
                overrides.push(OverrideDecl { name, proc_name, loc: oloc });
            }
        }

        self.expect(&TokenKind::End)?;
        Ok(TypeNode::Object { parent, fields, methods, overrides, loc })
    }

    fn parse_procedure_type(&mut self) -> CompileResult<TypeNode> {
        let loc = self.loc();
        self.expect(&TokenKind::Procedure)?;
        let mut params = Vec::new();
        let mut return_type = None;
        if self.eat(&TokenKind::LParen) {
            if !self.at(&TokenKind::RParen) {
                params = self.parse_proc_type_params()?;
            }
            self.expect(&TokenKind::RParen)?;
            if self.eat(&TokenKind::Colon) {
                return_type = Some(Box::new(self.parse_qual_type()?));
            }
        }
        Ok(TypeNode::ProcedureType { params, return_type, loc })
    }

    /// Parse procedure type parameter list - parameters may be unnamed (just types)
    fn parse_proc_type_params(&mut self) -> CompileResult<Vec<FormalParam>> {
        let mut params = Vec::new();
        loop {
            let loc = self.loc();
            let is_var = self.eat(&TokenKind::Var);

            // Check if this is "name : type" or just "type"
            // Save position and try to parse as named param
            let saved_pos = self.pos;
            if let TokenKind::Ident(_) = self.peek() {
                let name = self.expect_ident()?;
                if self.at(&TokenKind::Colon) {
                    // Named parameter: name : type
                    self.expect(&TokenKind::Colon)?;
                    let typ = self.parse_formal_type()?;
                    params.push(FormalParam {
                        is_var,
                        names: vec![name],
                        typ,
                        loc,
                    });
                } else if self.at(&TokenKind::Comma) {
                    // Could be "name, name, ... : type" (named) or "Type, Type" (unnamed)
                    // Look ahead past commas and idents to see if there's a ':' before ')' or ';'
                    let mut lookahead = self.pos;
                    let mut has_colon = false;
                    while lookahead < self.tokens.len() {
                        match &self.tokens[lookahead].kind {
                            TokenKind::Colon => { has_colon = true; break; }
                            TokenKind::RParen | TokenKind::Semi | TokenKind::Eof => break,
                            _ => lookahead += 1,
                        }
                    }
                    if has_colon {
                        // Multiple named params: name, name, ... : type
                        self.pos = saved_pos;
                        let fp = self.parse_formal_param()?;
                        params.push(fp);
                    } else {
                        // Unnamed parameter - just a type name (like INTEGER, INTEGER)
                        self.pos = saved_pos;
                        let typ = self.parse_formal_type()?;
                        params.push(FormalParam {
                            is_var,
                            names: vec![format!("_p{}", params.len())],
                            typ,
                            loc,
                        });
                    }
                } else {
                    // Unnamed parameter - just a type name
                    self.pos = saved_pos;
                    let typ = self.parse_formal_type()?;
                    params.push(FormalParam {
                        is_var,
                        names: vec![format!("_p{}", params.len())],
                        typ,
                        loc,
                    });
                }
            } else {
                let typ = self.parse_formal_type()?;
                params.push(FormalParam {
                    is_var,
                    names: vec![format!("_p{}", params.len())],
                    typ,
                    loc,
                });
            }
            if !self.eat(&TokenKind::Semi) && !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        Ok(params)
    }

    fn parse_enumeration_type(&mut self) -> CompileResult<TypeNode> {
        let loc = self.loc();
        self.expect(&TokenKind::LParen)?;
        let mut variants = vec![self.expect_ident()?];
        while self.eat(&TokenKind::Comma) {
            variants.push(self.expect_ident()?);
        }
        self.expect(&TokenKind::RParen)?;
        Ok(TypeNode::Enumeration { variants, loc })
    }

    // ── Statements ──────────────────────────────────────────────────

    fn parse_statement_sequence(&mut self) -> CompileResult<Vec<Statement>> {
        let mut stmts = Vec::new();
        match self.parse_statement() {
            Ok(s) => stmts.push(s),
            Err(e) => {
                self.record_error(e);
                self.synchronize();
            }
        }
        while self.eat(&TokenKind::Semi) {
            match self.parse_statement() {
                Ok(s) => stmts.push(s),
                Err(e) => {
                    self.record_error(e);
                    self.synchronize();
                }
            }
        }
        Ok(stmts)
    }

    fn parse_statement(&mut self) -> CompileResult<Statement> {
        let loc = self.loc();
        match self.peek() {
            TokenKind::Ident(_) => self.parse_assign_or_call(),
            TokenKind::If => self.parse_if(),
            TokenKind::Case => self.parse_case(),
            TokenKind::While => self.parse_while(),
            TokenKind::Repeat => self.parse_repeat(),
            TokenKind::For => self.parse_for(),
            TokenKind::Loop => self.parse_loop(),
            TokenKind::With => self.parse_with(),
            TokenKind::Return => self.parse_return(),
            TokenKind::Exit => {
                self.advance();
                Ok(Statement { kind: StatementKind::Exit, loc })
            }
            TokenKind::Retry => {
                self.advance();
                Ok(Statement { kind: StatementKind::Retry, loc })
            }
            TokenKind::Raise => {
                self.advance();
                let expr = if !self.at(&TokenKind::Semi) && !self.at(&TokenKind::End)
                    && !self.at(&TokenKind::Else) && !self.at(&TokenKind::Elsif)
                    && !self.at(&TokenKind::Pipe) {
                    Some(self.parse_expression()?)
                } else {
                    None
                };
                Ok(Statement { kind: StatementKind::Raise { expr }, loc })
            }
            TokenKind::Try => self.parse_try(),
            TokenKind::Lock => self.parse_lock(),
            TokenKind::Typecase => self.parse_typecase(),
            _ => Ok(Statement { kind: StatementKind::Empty, loc }),
        }
    }

    fn parse_assign_or_call(&mut self) -> CompileResult<Statement> {
        let loc = self.loc();
        let desig = self.parse_designator()?;
        if self.eat(&TokenKind::Assign) {
            let expr = self.parse_expression()?;
            Ok(Statement {
                kind: StatementKind::Assign { desig, expr },
                loc,
            })
        } else if self.eat(&TokenKind::LParen) {
            let mut args = Vec::new();
            if !self.at(&TokenKind::RParen) {
                args.push(self.parse_expression()?);
                while self.eat(&TokenKind::Comma) {
                    args.push(self.parse_expression()?);
                }
            }
            self.expect(&TokenKind::RParen)?;
            Ok(Statement {
                kind: StatementKind::ProcCall { desig, args },
                loc,
            })
        } else {
            // bare procedure call with no args
            Ok(Statement {
                kind: StatementKind::ProcCall { desig, args: vec![] },
                loc,
            })
        }
    }

    fn parse_if(&mut self) -> CompileResult<Statement> {
        let loc = self.loc();
        self.expect(&TokenKind::If)?;
        let cond = self.parse_expression()?;
        self.expect(&TokenKind::Then)?;
        let then_body = self.parse_statement_sequence()?;
        let mut elsifs = Vec::new();
        while self.eat(&TokenKind::Elsif) {
            let c = self.parse_expression()?;
            self.expect(&TokenKind::Then)?;
            let b = self.parse_statement_sequence()?;
            elsifs.push((c, b));
        }
        let else_body = if self.eat(&TokenKind::Else) {
            Some(self.parse_statement_sequence()?)
        } else {
            None
        };
        self.expect(&TokenKind::End)?;
        Ok(Statement {
            kind: StatementKind::If { cond, then_body, elsifs, else_body },
            loc,
        })
    }

    fn parse_case(&mut self) -> CompileResult<Statement> {
        let loc = self.loc();
        self.expect(&TokenKind::Case)?;
        let expr = self.parse_expression()?;
        self.expect(&TokenKind::Of)?;
        let mut branches = Vec::new();
        if !self.at(&TokenKind::End) && !self.at(&TokenKind::Else) {
            branches.push(self.parse_case_branch()?);
            while self.eat(&TokenKind::Pipe) {
                branches.push(self.parse_case_branch()?);
            }
        }
        let else_body = if self.eat(&TokenKind::Else) {
            Some(self.parse_statement_sequence()?)
        } else {
            None
        };
        self.expect(&TokenKind::End)?;
        Ok(Statement {
            kind: StatementKind::Case { expr, branches, else_body },
            loc,
        })
    }

    fn parse_case_branch(&mut self) -> CompileResult<CaseBranch> {
        let loc = self.loc();
        let labels = self.parse_case_labels()?;
        self.expect(&TokenKind::Colon)?;
        let body = self.parse_statement_sequence()?;
        Ok(CaseBranch { labels, body, loc })
    }

    fn parse_case_labels(&mut self) -> CompileResult<Vec<CaseLabel>> {
        let mut labels = vec![self.parse_case_label()?];
        while self.eat(&TokenKind::Comma) {
            labels.push(self.parse_case_label()?);
        }
        Ok(labels)
    }

    fn parse_case_label(&mut self) -> CompileResult<CaseLabel> {
        let low = self.parse_expression()?;
        if self.eat(&TokenKind::DotDot) {
            let high = self.parse_expression()?;
            Ok(CaseLabel::Range(low, high))
        } else {
            Ok(CaseLabel::Single(low))
        }
    }

    fn parse_while(&mut self) -> CompileResult<Statement> {
        let loc = self.loc();
        self.expect(&TokenKind::While)?;
        let cond = self.parse_expression()?;
        self.expect(&TokenKind::Do)?;
        let body = self.parse_statement_sequence()?;
        self.expect(&TokenKind::End)?;
        Ok(Statement {
            kind: StatementKind::While { cond, body },
            loc,
        })
    }

    fn parse_repeat(&mut self) -> CompileResult<Statement> {
        let loc = self.loc();
        self.expect(&TokenKind::Repeat)?;
        let body = self.parse_statement_sequence()?;
        self.expect(&TokenKind::Until)?;
        let cond = self.parse_expression()?;
        Ok(Statement {
            kind: StatementKind::Repeat { body, cond },
            loc,
        })
    }

    fn parse_for(&mut self) -> CompileResult<Statement> {
        let loc = self.loc();
        self.expect(&TokenKind::For)?;
        let var = self.expect_ident()?;
        self.expect(&TokenKind::Assign)?;
        let start = self.parse_expression()?;
        self.expect(&TokenKind::To)?;
        let end = self.parse_expression()?;
        let step = if self.eat(&TokenKind::By) {
            Some(self.parse_expression()?)
        } else {
            None
        };
        self.expect(&TokenKind::Do)?;
        let body = self.parse_statement_sequence()?;
        self.expect(&TokenKind::End)?;
        Ok(Statement {
            kind: StatementKind::For { var, start, end, step, body },
            loc,
        })
    }

    fn parse_loop(&mut self) -> CompileResult<Statement> {
        let loc = self.loc();
        self.expect(&TokenKind::Loop)?;
        let body = self.parse_statement_sequence()?;
        self.expect(&TokenKind::End)?;
        Ok(Statement {
            kind: StatementKind::Loop { body },
            loc,
        })
    }

    fn parse_with(&mut self) -> CompileResult<Statement> {
        let loc = self.loc();
        self.expect(&TokenKind::With)?;
        let desig = self.parse_designator()?;
        self.expect(&TokenKind::Do)?;
        let body = self.parse_statement_sequence()?;
        self.expect(&TokenKind::End)?;
        Ok(Statement {
            kind: StatementKind::With { desig, body },
            loc,
        })
    }

    fn parse_return(&mut self) -> CompileResult<Statement> {
        let loc = self.loc();
        self.expect(&TokenKind::Return)?;
        // RETURN may optionally have an expression
        let expr = match self.peek() {
            TokenKind::Semi | TokenKind::End | TokenKind::Else
            | TokenKind::Elsif | TokenKind::Until | TokenKind::Pipe
            | TokenKind::Eof => None,
            _ => Some(self.parse_expression()?),
        };
        Ok(Statement {
            kind: StatementKind::Return { expr },
            loc,
        })
    }

    // ── Expressions ─────────────────────────────────────────────────
    // Precedence (low to high):
    // 1: OR
    // 2: AND / &
    // 3: NOT / ~  (unary, handled in factor)
    // 4: = # <> < <= > >= IN  (relational)
    // 5: + -  (additive)
    // 6: * / DIV MOD  (multiplicative)
    // 7: unary + -
    // 8: factor (literals, designators, function calls, parens, set constructors)

    pub fn parse_expression(&mut self) -> CompileResult<Expr> {
        self.parse_or_expr()
    }

    fn parse_or_expr(&mut self) -> CompileResult<Expr> {
        let mut left = self.parse_and_expr()?;
        while self.at(&TokenKind::Or) {
            let loc = self.loc();
            self.advance();
            let right = self.parse_and_expr()?;
            left = Expr {
                kind: ExprKind::BinaryOp {
                    op: BinaryOp::Or,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                loc,
            };
        }
        Ok(left)
    }

    fn parse_and_expr(&mut self) -> CompileResult<Expr> {
        let mut left = self.parse_rel_expr()?;
        loop {
            match self.peek() {
                TokenKind::And | TokenKind::Ampersand => {
                    let loc = self.loc();
                    self.advance();
                    let right = self.parse_rel_expr()?;
                    left = Expr {
                        kind: ExprKind::BinaryOp {
                            op: BinaryOp::And,
                            left: Box::new(left),
                            right: Box::new(right),
                        },
                        loc,
                    };
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_rel_expr(&mut self) -> CompileResult<Expr> {
        let left = self.parse_add_expr()?;
        let op = match self.peek() {
            TokenKind::Eq => Some(BinaryOp::Eq),
            TokenKind::Hash | TokenKind::NotEq => Some(BinaryOp::Ne),
            TokenKind::Lt => Some(BinaryOp::Lt),
            TokenKind::Le => Some(BinaryOp::Le),
            TokenKind::Gt => Some(BinaryOp::Gt),
            TokenKind::Ge => Some(BinaryOp::Ge),
            TokenKind::In => Some(BinaryOp::In),
            _ => None,
        };
        if let Some(op) = op {
            let loc = self.loc();
            self.advance();
            let right = self.parse_add_expr()?;
            Ok(Expr {
                kind: ExprKind::BinaryOp {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                loc,
            })
        } else {
            Ok(left)
        }
    }

    fn parse_add_expr(&mut self) -> CompileResult<Expr> {
        let mut left = self.parse_mul_expr()?;
        loop {
            let op = match self.peek() {
                TokenKind::Plus => Some(BinaryOp::Add),
                TokenKind::Minus => Some(BinaryOp::Sub),
                _ => None,
            };
            if let Some(op) = op {
                let loc = self.loc();
                self.advance();
                let right = self.parse_mul_expr()?;
                left = Expr {
                    kind: ExprKind::BinaryOp {
                        op,
                        left: Box::new(left),
                        right: Box::new(right),
                    },
                    loc,
                };
            } else {
                break;
            }
        }
        Ok(left)
    }

    fn parse_mul_expr(&mut self) -> CompileResult<Expr> {
        let mut left = self.parse_unary_expr()?;
        loop {
            let op = match self.peek() {
                TokenKind::Star => Some(BinaryOp::Mul),
                TokenKind::Slash => Some(BinaryOp::RealDiv),
                TokenKind::Div => Some(BinaryOp::IntDiv),
                TokenKind::Mod => Some(BinaryOp::Mod),
                _ => None,
            };
            if let Some(op) = op {
                let loc = self.loc();
                self.advance();
                let right = self.parse_unary_expr()?;
                left = Expr {
                    kind: ExprKind::BinaryOp {
                        op,
                        left: Box::new(left),
                        right: Box::new(right),
                    },
                    loc,
                };
            } else {
                break;
            }
        }
        Ok(left)
    }

    fn parse_unary_expr(&mut self) -> CompileResult<Expr> {
        match self.peek() {
            TokenKind::Plus => {
                let loc = self.loc();
                self.advance();
                let operand = self.parse_factor()?;
                Ok(Expr {
                    kind: ExprKind::UnaryOp {
                        op: UnaryOp::Pos,
                        operand: Box::new(operand),
                    },
                    loc,
                })
            }
            TokenKind::Minus => {
                let loc = self.loc();
                self.advance();
                let operand = self.parse_factor()?;
                Ok(Expr {
                    kind: ExprKind::UnaryOp {
                        op: UnaryOp::Neg,
                        operand: Box::new(operand),
                    },
                    loc,
                })
            }
            TokenKind::Not | TokenKind::Tilde => {
                let loc = self.loc();
                self.advance();
                let operand = self.parse_factor()?;
                Ok(Expr {
                    kind: ExprKind::Not(Box::new(operand)),
                    loc,
                })
            }
            _ => self.parse_factor(),
        }
    }

    fn parse_factor(&mut self) -> CompileResult<Expr> {
        let loc = self.loc();
        match self.peek().clone() {
            TokenKind::IntLit(v) => {
                self.advance();
                Ok(Expr { kind: ExprKind::IntLit(v), loc })
            }
            TokenKind::RealLit(v) => {
                self.advance();
                Ok(Expr { kind: ExprKind::RealLit(v), loc })
            }
            TokenKind::StringLit(s) => {
                self.advance();
                Ok(Expr { kind: ExprKind::StringLit(s), loc })
            }
            TokenKind::CharLit(c) => {
                self.advance();
                Ok(Expr { kind: ExprKind::CharLit(c), loc })
            }
            TokenKind::LParen => {
                self.advance();
                let e = self.parse_expression()?;
                self.expect(&TokenKind::RParen)?;
                Ok(e)
            }
            TokenKind::LBrace => {
                self.parse_set_constructor(None)
            }
            TokenKind::Ident(_) => {
                let desig = self.parse_designator()?;
                // Check for function call or set constructor
                if self.at(&TokenKind::LParen) {
                    self.advance();
                    let mut args = Vec::new();
                    if !self.at(&TokenKind::RParen) {
                        args.push(self.parse_expression()?);
                        while self.eat(&TokenKind::Comma) {
                            args.push(self.parse_expression()?);
                        }
                    }
                    self.expect(&TokenKind::RParen)?;
                    Ok(Expr {
                        kind: ExprKind::FuncCall { desig, args },
                        loc,
                    })
                } else if self.at(&TokenKind::LBrace) {
                    // Set constructor with type: TypeName{...}
                    let qi = desig.ident.clone();
                    self.parse_set_constructor(Some(qi))
                } else {
                    Ok(Expr {
                        kind: ExprKind::Designator(desig),
                        loc,
                    })
                }
            }
            _ => Err(CompileError::parser(
                loc,
                format!("expected expression, found {}", self.peek().describe()),
            )),
        }
    }

    fn parse_set_constructor(&mut self, base: Option<QualIdent>) -> CompileResult<Expr> {
        let loc = self.loc();
        self.expect(&TokenKind::LBrace)?;
        let mut elements = Vec::new();
        if !self.at(&TokenKind::RBrace) {
            elements.push(self.parse_set_element()?);
            while self.eat(&TokenKind::Comma) {
                elements.push(self.parse_set_element()?);
            }
        }
        self.expect(&TokenKind::RBrace)?;
        Ok(Expr {
            kind: ExprKind::SetConstructor {
                base_type: base,
                elements,
            },
            loc,
        })
    }

    fn parse_set_element(&mut self) -> CompileResult<SetElement> {
        let e = self.parse_expression()?;
        if self.eat(&TokenKind::DotDot) {
            let high = self.parse_expression()?;
            Ok(SetElement::Range(e, high))
        } else {
            Ok(SetElement::Single(e))
        }
    }

    // ── Designator ──────────────────────────────────────────────────

    fn parse_designator(&mut self) -> CompileResult<Designator> {
        let loc = self.loc();
        let name = self.expect_ident()?;
        let ident = QualIdent {
            module: None,
            name,
            loc: loc.clone(),
        };
        let mut selectors = Vec::new();
        loop {
            match self.peek() {
                TokenKind::Dot => {
                    let sloc = self.loc();
                    self.advance();
                    let field = self.expect_ident()?;
                    selectors.push(Selector::Field(field, sloc));
                }
                TokenKind::LBrack => {
                    let sloc = self.loc();
                    self.advance();
                    let mut indices = vec![self.parse_expression()?];
                    while self.eat(&TokenKind::Comma) {
                        indices.push(self.parse_expression()?);
                    }
                    self.expect(&TokenKind::RBrack)?;
                    selectors.push(Selector::Index(indices, sloc));
                }
                TokenKind::Caret => {
                    let sloc = self.loc();
                    self.advance();
                    selectors.push(Selector::Deref(sloc));
                }
                _ => break,
            }
        }
        Ok(Designator { ident, selectors, loc })
    }

    // ── Modula-2+ statements ─────────────────────────────────────────

    /// Parse TRY body {EXCEPT [ExcName [(var)]] => stmts} [FINALLY stmts] END
    fn parse_try(&mut self) -> CompileResult<Statement> {
        let loc = self.loc();
        self.expect(&TokenKind::Try)?;
        let body = self.parse_statement_sequence()?;
        let mut excepts = Vec::new();
        while self.eat(&TokenKind::Except) {
            let eloc = self.loc();
            // Named handler: EXCEPT ExcName [(var)] DO stmts
            // Catch-all:     EXCEPT stmts
            // Distinguish by look-ahead: if Ident followed by DO or LParen-Ident-RParen-DO,
            // it's a named handler. Otherwise it's a catch-all.
            let exception = if self.at(&TokenKind::Ident(String::new())) {
                let saved = self.pos;
                let name = self.expect_ident()?;
                if self.at(&TokenKind::Do) {
                    // Named handler without var binding
                    let qi = QualIdent { module: None, name, loc: eloc.clone() };
                    Some(qi)
                } else if self.at(&TokenKind::LParen) {
                    // Check: ( Ident ) DO  = named handler with var
                    // But ( expr... could be a proc call = catch-all body
                    let saved2 = self.pos;
                    self.advance(); // consume (
                    if let TokenKind::Ident(_) = self.peek() {
                        let var_name = self.expect_ident()?;
                        if self.at(&TokenKind::RParen) {
                            self.advance(); // consume )
                            if self.at(&TokenKind::Do) {
                                // Confirmed: named handler with var binding
                                let qi = QualIdent { module: None, name, loc: eloc.clone() };
                                // Don't consume DO yet (parsed below)
                                // But we need to remember var_name
                                // Actually, let me restructure this:
                                // Return and parse DO below
                                self.pos = saved;
                                let qi_name = self.expect_ident()?;
                                let qi = QualIdent { module: None, name: qi_name, loc: eloc.clone() };
                                Some(qi)
                            } else {
                                // Not DO after ) — rewind to catch-all
                                self.pos = saved;
                                None
                            }
                        } else {
                            // Not RParen — rewind to catch-all
                            self.pos = saved;
                            None
                        }
                    } else {
                        // Not Ident after ( — rewind to catch-all
                        self.pos = saved;
                        None
                    }
                } else {
                    // Not DO or ( — rewind to catch-all
                    self.pos = saved;
                    None
                }
            } else {
                None
            };
            let var = if exception.is_some() && self.eat(&TokenKind::LParen) {
                let v = self.expect_ident()?;
                self.expect(&TokenKind::RParen)?;
                Some(v)
            } else {
                None
            };
            // Consume DO if present (after named exception handler)
            if exception.is_some() {
                self.eat(&TokenKind::Do);
            }
            let handler_body = self.parse_statement_sequence()?;
            excepts.push(ExceptClause {
                exception,
                var,
                body: handler_body,
                loc: eloc,
            });
        }
        let finally_body = if self.eat(&TokenKind::Finally) {
            Some(self.parse_statement_sequence()?)
        } else {
            None
        };
        self.expect(&TokenKind::End)?;
        Ok(Statement {
            kind: StatementKind::Try { body, excepts, finally_body },
            loc,
        })
    }

    /// Parse LOCK expr DO stmts END
    fn parse_lock(&mut self) -> CompileResult<Statement> {
        let loc = self.loc();
        self.expect(&TokenKind::Lock)?;
        let mutex = self.parse_expression()?;
        self.expect(&TokenKind::Do)?;
        let body = self.parse_statement_sequence()?;
        self.expect(&TokenKind::End)?;
        Ok(Statement {
            kind: StatementKind::Lock { mutex, body },
            loc,
        })
    }

    /// Parse TYPECASE expr OF Type1 [(var1)] => stmts | Type2 => stmts [ELSE stmts] END
    fn parse_typecase(&mut self) -> CompileResult<Statement> {
        let loc = self.loc();
        self.expect(&TokenKind::Typecase)?;
        let expr = self.parse_expression()?;
        self.expect(&TokenKind::Of)?;
        let mut branches = Vec::new();
        if !self.at(&TokenKind::End) && !self.at(&TokenKind::Else) {
            branches.push(self.parse_typecase_branch()?);
            while self.eat(&TokenKind::Pipe) {
                branches.push(self.parse_typecase_branch()?);
            }
        }
        let else_body = if self.eat(&TokenKind::Else) {
            Some(self.parse_statement_sequence()?)
        } else {
            None
        };
        self.expect(&TokenKind::End)?;
        Ok(Statement {
            kind: StatementKind::TypeCase { expr, branches, else_body },
            loc,
        })
    }

    fn parse_typecase_branch(&mut self) -> CompileResult<TypeCaseBranch> {
        let loc = self.loc();
        let mut types = vec![self.parse_qualident()?];
        while self.eat(&TokenKind::Comma) {
            types.push(self.parse_qualident()?);
        }
        let var = if self.eat(&TokenKind::LParen) {
            let v = self.expect_ident()?;
            self.expect(&TokenKind::RParen)?;
            Some(v)
        } else {
            None
        };
        // Expect => (which is just = then > in separate tokens, or we use colon)
        // For simplicity, use colon as separator like CASE branches
        self.expect(&TokenKind::Colon)?;
        let body = self.parse_statement_sequence()?;
        Ok(TypeCaseBranch { types, var, body, loc })
    }

    fn parse_qualident(&mut self) -> CompileResult<QualIdent> {
        let loc = self.loc();
        let first = self.expect_ident()?;
        if self.at(&TokenKind::Dot) {
            // Check if next after dot is an ident (qualified) vs something else
            // We need to be careful: dot could be field selector
            // In a qualident context, Module.Name, we only do one level
            if let Some(Token { kind: TokenKind::Ident(_), .. }) = self.tokens.get(self.pos + 1) {
                // Could be qualified ident or could be designator.field
                // We only treat as qualified in qualident parse context
                self.advance(); // consume dot
                let name = self.expect_ident()?;
                return Ok(QualIdent { module: Some(first), name, loc });
            }
        }
        Ok(QualIdent { module: None, name: first, loc })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    fn parse(input: &str) -> CompilationUnit {
        let mut lexer = Lexer::new(input, "test");
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        parser.parse_compilation_unit().unwrap()
    }

    #[test]
    fn test_empty_module() {
        let cu = parse("MODULE Test; END Test.");
        match cu {
            CompilationUnit::ProgramModule(m) => {
                assert_eq!(m.name, "Test");
                assert!(m.block.body.is_none());
            }
            _ => panic!("expected program module"),
        }
    }

    #[test]
    fn test_module_with_import() {
        let cu = parse("MODULE Test; FROM InOut IMPORT WriteString, WriteLn; BEGIN END Test.");
        match cu {
            CompilationUnit::ProgramModule(m) => {
                assert_eq!(m.imports.len(), 1);
                assert_eq!(m.imports[0].from_module, Some("InOut".to_string()));
                let names: Vec<&str> = m.imports[0].names.iter().map(|n| n.name.as_str()).collect();
                assert_eq!(names, vec!["WriteString", "WriteLn"]);
            }
            _ => panic!("expected program module"),
        }
    }

    #[test]
    fn test_import_as_alias() {
        let cu = parse("MODULE Test; FROM InOut IMPORT WriteString AS WS, WriteLn; BEGIN END Test.");
        match cu {
            CompilationUnit::ProgramModule(m) => {
                assert_eq!(m.imports.len(), 1);
                assert_eq!(m.imports[0].from_module, Some("InOut".to_string()));
                assert_eq!(m.imports[0].names.len(), 2);
                // First import has alias
                assert_eq!(m.imports[0].names[0].name, "WriteString");
                assert_eq!(m.imports[0].names[0].alias, Some("WS".to_string()));
                assert_eq!(m.imports[0].names[0].local_name(), "WS");
                // Second import has no alias
                assert_eq!(m.imports[0].names[1].name, "WriteLn");
                assert_eq!(m.imports[0].names[1].alias, None);
                assert_eq!(m.imports[0].names[1].local_name(), "WriteLn");
            }
            _ => panic!("expected program module"),
        }
    }

    #[test]
    fn test_hello_world() {
        let src = r#"
MODULE Hello;
FROM InOut IMPORT WriteString, WriteLn;
BEGIN
  WriteString("Hello, World!");
  WriteLn
END Hello.
"#;
        let cu = parse(src);
        match cu {
            CompilationUnit::ProgramModule(m) => {
                assert_eq!(m.name, "Hello");
                let body = m.block.body.as_ref().unwrap();
                assert_eq!(body.len(), 2);
            }
            _ => panic!("expected program module"),
        }
    }

    #[test]
    fn test_definition_module() {
        let src = r#"
DEFINITION MODULE Stack;
  EXPORT QUALIFIED Push, Pop, Item;
  TYPE Item;
  PROCEDURE Push(x: Item);
  PROCEDURE Pop(): Item;
END Stack.
"#;
        let cu = parse(src);
        match cu {
            CompilationUnit::DefinitionModule(m) => {
                assert_eq!(m.name, "Stack");
                assert!(m.export.is_some());
            }
            _ => panic!("expected definition module"),
        }
    }
}
