# Compilation Pipeline

```mermaid
flowchart TB
    subgraph Input
        SRC["Source (.mod/.def)"]
    end

    subgraph "Phase 1: Parse .def files"
        SRC --> LEX1["Lexer"]
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

    HIRMOD --> BACKEND{"Backend\nselection"}

    subgraph "C Backend"
        BACKEND -->|"default"| CHIR["Iterate HirModule\n(types, consts, globals,\nproc fwd decls)"]
        CHIR --> CEMIT["hir_emit.rs\nHIR → C text"]
        CEMIT --> CCODE[".c file"]
        CCODE --> CC["cc / clang"]
        CC --> BIN_C["Executable"]
    end

    subgraph "LLVM Backend"
        BACKEND -->|"--llvm"| LHIR["Iterate HirModule\n(types, consts, globals,\nexceptions, proc_decls,\nbody/except/finally)"]
        LHIR --> LEMIT["decls.rs + stmts.rs + exprs.rs\nHIR → LLVM IR"]
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
    style CHIR fill:#f9f,stroke:#333
    style LHIR fill:#f9f,stroke:#333
    style SEMAFINAL fill:#ff9,stroke:#333
    style RESULT fill:#9ff,stroke:#333
```

## Key Points

- **Single sema, shared by both backends.** Sema runs once; both C and LLVM backends read the same symtab, types, and scope chain.
- **Prebuilt HirModule is the primary data source.** `build_module()` constructs an `HirModule` after sema, containing structural declarations (types, consts, globals, proc signatures, embedded modules) and pre-lowered statement bodies. Both backends iterate from HirModule for structural emission.
- **HIR is the single codegen path for all statement/expression bodies.** Statement and expression lowering resolves designators, expands open array arguments, desugars WITH, and registers TYPECASE bindings. No AST walking remains in body codegen for either backend.
- **TypeId → C name resolver.** A `typeid_c_names` map resolves TypeIds to C typedef names, populated incrementally from HirModule type_decls, def-module registration, and gen_type_decl emission. Only non-structural types (records, enums, arrays, aliases) are registered to avoid cross-module pointer-type name conflicts.
- **Both backends are fully HIR-driven.** Neither backend walks the AST for codegen. The C backend uses `type_id_to_c()` and `field_type_and_suffix()` for TypeId-based type resolution. The LLVM backend uses `tl_type_str()` from type_lowering and `llvm_type_for_type_id()` — both resolve from TypeIds, not AST TypeNodes.
- **Phase 3 uses full analysis** (`register_impl_module` → `analyze_implementation_module`) so that procedure parameters, local variables, and constants in embedded modules are all registered in sema's scope chain. The HIR builder depends on this.
- **Def modules are topologically sorted** (Phase 2) and recursively registered (Phase 3) so that cross-module type references (e.g., `URIRec` from `URI.def` used by `HTTPClient.def`) resolve in the correct order.
- **M2+ exception handling** uses setjmp/longjmp-based `m2_ExcFrame` stack in both backends. The C backend emits `M2_TRY`/`M2_CATCH` macros; the LLVM backend calls `m2_exc_push`/`setjmp`/`m2_exc_pop` runtime functions directly.
- **LSP skips codegen entirely.** The analysis-only path (`analyze_source`) produces the same sema artifacts without generating C or LLVM IR.

## Module Structure

```
src/
  driver.rs              Pipeline orchestration (Phases 1-5, backend dispatch)
  lexer.rs               Tokenizer
  parser.rs              Recursive-descent → AST
  ast.rs                 AST node types
  sema.rs                Semantic analysis (type checking, scope resolution)
  symtab.rs              Symbol table (scoped, nested)
  types.rs               Type registry
  hir.rs                 HIR types (Place, HirExpr, HirStmt, HirModule, HirProcDecl, etc.)
  hir_build.rs           build_module() + HirBuilder (designator resolution, call expansion)
  analyze.rs             LSP analysis-only path
  build.rs               mx build/run/test subcommands
  codegen_c/
    mod.rs               C backend core
    modules.rs           Module-level codegen, embedded impl modules
    decls.rs             Procedure/variable declarations
    stmts.rs             Statement dispatch (routes all to HIR)
    hir_emit.rs          HIR → C emission (all statements + expressions)
    exprs.rs             Legacy helpers (escape functions)
    designators.rs       HIR Place → C designator strings
    types.rs             Type → C type string mapping
    m2plus.rs            M2+ type/declaration codegen (REF, OBJECT, EXCEPTION)
  codegen_llvm/
    mod.rs               LLVM backend core (registration APIs, generate entry)
    modules.rs           Module-level codegen (all HIR-driven, zero AST deps)
    decls.rs             HIR-based type/const/var/proc emission
    stmts.rs             HIR → LLVM IR statements (PIM4 + M2+ exceptions)
    exprs.rs             HIR → LLVM IR expressions, short-circuit, COMPLEX builtins
    designators.rs       HIR Place → LLVM IR address/load (variant field offsets)
    types.rs             TypeId resolution, type coercion
    type_lowering.rs     M2 types → LLVM IR types
    llvm_types.rs        LLVM type representation
    stdlib_sigs.rs       Standard library call signatures
    debug_info.rs        DWARF metadata
    closures.rs          (removed — captures now in HirProcDecl.closure_captures)
```
