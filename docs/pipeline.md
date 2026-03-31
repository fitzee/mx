# Compilation Pipeline

```mermaid
flowchart TB
    subgraph Input
        SRC["Source (.mod/.def)"]
    end

    subgraph "Phase 0: Target detection"
        SRC --> TARGET["TargetInfo\n(from --target or host)"]
    end

    subgraph "Phase 1: Parse .def files"
        TARGET --> LEX1["Lexer"]
        LEX1 --> PAR1["Parser"]
        PAR1 --> DEFS["parsed_defs\nHashMap&lt;String, DefinitionModule&gt;"]
    end

    subgraph "Phase 2: Register .def modules"
        DEFS --> TOPO["Topological Sort\n(by import deps)"]
        TOPO --> PREREG["Pre-register Type Names\n(Opaque placeholders)"]
        PREREG --> DEFREG["register_def_module()\nfor each .def in order"]
        DEFREG --> SEMA1["Sema\n(types + exported symbols)"]
    end

    subgraph "Phase 3: Load .mod files"
        SEMA1 --> MODFIND["Find .mod files\n(transitive imports)"]
        MODFIND --> DEFRECUR["register_def_recursive()\nfor new .def dependencies"]
        DEFRECUR --> MODREG["register_impl_module()\nfull analyze for each .mod"]
        MODREG --> SEMA2["Sema\n(+ all embedded module scopes)"]
    end

    subgraph "Phase 4: Analyze main unit"
        SRC --> LEX2["Lexer"]
        LEX2 --> PAR2["Parser"]
        PAR2 --> AST["AST\n(CompilationUnit)"]
        SEMA2 --> ANALYZE["sema.analyze(unit)\nscope_map, ref_index, call_graph"]
        AST --> ANALYZE
        ANALYZE --> SEMA3["Sema Complete\n(symtab + types + scopes)"]
    end

    subgraph "Phase 5: Fixup"
        SEMA3 --> FIXUP["fixup_record_field_types()\nresolve Opaque → concrete"]
        FIXUP --> SEMAFINAL["Final Sema"]
    end

    subgraph "Phase 6: Build prebuilt HIR"
        SEMAFINAL --> HIRMOD["build_module()\nHirModule with structural decls,\nlowered stmts, proc bodies,\nembedded modules"]
    end

    subgraph "Phase 7: Build CFGs"
        HIRMOD --> CFG["build_cfg()\nfor each proc body,\nmodule init, embedded init\n(with except/finally wrapping)"]
        CFG --> CFGOUT["Pre-built Cfg on each\nHirProcDecl, HirModule,\nHirEmbeddedModule"]
    end

    CFGOUT --> BACKEND{"Backend\nselection"}
    TARGET -.->|"&TargetInfo"| BACKEND

    subgraph "C Backend"
        BACKEND -->|"default"| CHIR["Iterate HirModule\n(types, consts, globals,\nproc fwd decls)"]
        CHIR --> CEMIT["cfg_emit.rs\nCFG blocks → goto-based C\n+ SJLJ handler regions\n+ layout guards"]
        CEMIT --> CCODE[".c file"]
        CCODE --> CC["cc / clang\n(target-aware flags)"]
        CC --> BIN_C["Executable"]
    end

    subgraph "LLVM Backend"
        BACKEND -->|"--llvm"| LHIR["Iterate HirModule\n(types, consts, globals,\nexceptions, proc_decls)"]
        LHIR --> LEMIT["cfg_emit.rs\nCFG blocks → LLVM basic blocks\n+ SJLJ handler regions\n(target triple + datalayout)"]
        LEMIT --> LLCODE[".ll file"]
        LLCODE --> CLANG["clang"]
        CLANG --> BIN_L["Executable"]
    end

    subgraph "LSP Path (no codegen)"
        SRC --> LEX3["Lexer"]
        LEX3 --> PAR3["Parser"]
        PAR3 --> AST3["AST"]
        AST3 --> LSPANA["analyze_source()\nlex → parse → sema only"]
        LSPANA --> RESULT["AnalysisResult\n(symtab, types, scope_map,\nref_index, call_graph)"]
    end

    style HIRMOD fill:#f9f,stroke:#333
    style CFGOUT fill:#f9f,stroke:#333
    style CHIR fill:#f9f,stroke:#333
    style LHIR fill:#f9f,stroke:#333
    style SEMAFINAL fill:#ff9,stroke:#333
    style RESULT fill:#9ff,stroke:#333
```

## Key Points

- **Target-first.** `TargetInfo` is constructed before any compilation — from `--target` or host detection. Both backends receive `&TargetInfo` and use it for target-specific output (LLVM triple/datalayout, C layout guards, linker flags).
- **Single sema, shared by both backends.** Sema runs once; both C and LLVM backends read the same symtab, types, and scope chain.
- **Prebuilt HirModule is the primary data source.** `build_module()` constructs an `HirModule` after sema, containing structural declarations (types, consts, globals, proc signatures, embedded modules) and pre-lowered statement bodies. Both backends iterate from HirModule for structural emission.
- **CFG is the single source of truth for control flow.** After HIR construction, all procedure and init bodies are lowered to CFGs (`build_cfg`). Both backends iterate CFG blocks in order, emitting labels, statements, and terminators. No backend reconstructs structured control flow from HIR — IF/WHILE/CASE/LOOP/FOR/TRY are all represented as CFG Goto/Branch/Switch/Return/Raise terminators.
- **HIR provides expressions and simple statements.** CFG blocks contain `HirStmt` (only Assign and ProcCall). Expression emission uses HIR's `HirExpr` tree. Designator resolution, open array expansion, WITH desugaring, and TYPECASE bindings are all handled by the HIR builder before CFG construction.
- **TypeId → C name resolver.** A `typeid_c_names` map resolves TypeIds to C typedef names, populated incrementally from HirModule type_decls, def-module registration, and gen_type_decl emission. Only non-structural types (records, enums, arrays, aliases) are registered to avoid cross-module pointer-type name conflicts.
- **Phase 3 uses full analysis** (`register_impl_module` → `analyze_implementation_module`) so that procedure parameters, local variables, and constants in embedded modules are all registered in sema's scope chain. The HIR builder depends on this.
- **Def modules are topologically sorted** (Phase 2) and recursively registered (Phase 3) so that cross-module type references (e.g., `URIRec` from `URI.def` used by `HTTPClient.def`) resolve in the correct order.
- **M2+ exception handling** uses setjmp/longjmp-based `m2_ExcFrame` stack. The CFG annotates blocks with `handler: Option<BlockId>` for exception regions. Both backends detect handler transitions during block emission and emit SJLJ frame setup/teardown inline. Proc-level EXCEPT and module-level FINALLY are folded into the CFG via synthetic TRY wrapping in the driver.
- **LSP skips codegen entirely.** The analysis-only path (`analyze_source`) produces the same sema artifacts without generating C or LLVM IR.

## Module Structure

```
src/
  target.rs              Target abstraction (TargetInfo, layout computation, ABI)
  driver.rs              Pipeline orchestration (Phases 1-5, backend dispatch)
  lexer.rs               Tokenizer
  parser.rs              Recursive-descent → AST
  ast.rs                 AST node types
  sema.rs                Semantic analysis (type checking, scope resolution)
  symtab.rs              Symbol table (scoped, nested)
  types.rs               Type registry
  hir.rs                 HIR types (Place, HirExpr, HirStmt, HirModule, HirProcDecl, etc.)
  hir_build/
    mod.rs               build_module(), struct defs, helpers, tests
    lower.rs             HirBuilder impl (designator resolution, expr/stmt lowering, call expansion)
  cfg/
    mod.rs               Data model (BasicBlock, Terminator, Cfg), verify, cleanup, DOT output
    build.rs             CfgBuilder, build_cfg(), all control-flow lowering
  analyze.rs             LSP analysis-only path
  build.rs               mx build/run/test subcommands
  codegen_c/
    mod.rs               C backend core
    cfg_emit.rs          CFG-driven body emission (block labels, goto terminators, SJLJ handlers)
    modules.rs           Module-level codegen, embedded impl modules
    decls.rs             Procedure/variable declarations
    stmts.rs             Statement dispatch (routes Assign/ProcCall to HIR emitter)
    hir_emit.rs          HIR → C emission for Assign/ProcCall statements and expressions
    exprs.rs             Legacy helpers (escape functions)
    designators.rs       HIR Place → C designator strings
    types.rs             Type → C type string mapping
    m2plus.rs            M2+ type/declaration codegen (REF, OBJECT, EXCEPTION)
  codegen_llvm/
    mod.rs               LLVM backend core (registration APIs, generate entry)
    cfg_emit.rs          CFG-driven body emission (LLVM basic blocks, br terminators, SJLJ handlers)
    modules.rs           Module-level codegen (all HIR-driven, zero AST deps)
    decls.rs             HIR-based type/const/var/proc emission
    stmts.rs             HIR → LLVM IR statements (Assign/ProcCall only)
    exprs.rs             HIR → LLVM IR expressions, COMPLEX builtins
    designators.rs       HIR Place → LLVM IR address/load (variant field offsets)
    types.rs             TypeId resolution, type coercion
    type_lowering.rs     M2 types → LLVM IR types
    llvm_types.rs        LLVM type representation
    stdlib_sigs.rs       Standard library call signatures
    debug_info.rs        DWARF metadata
```
