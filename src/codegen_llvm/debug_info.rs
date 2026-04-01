/// LLVM debug info (DWARF) metadata emission.
///
/// Emits !DICompileUnit, !DIFile, !DISubprogram, and !DILocation
/// metadata so debuggers can map instructions back to M2 source lines.
/// Only active when --debug / -g is set.

use std::collections::HashMap;

/// Accumulated debug metadata for an LLVM IR module.
pub(crate) struct DebugInfoBuilder {
    /// Next metadata node ID
    next_id: usize,
    /// Accumulated metadata lines: "!N = ..."
    metadata: Vec<String>,
    /// File path → metadata ID for !DIFile
    files: HashMap<String, usize>,
    /// Compile unit metadata ID
    compile_unit: Option<usize>,
    /// Current scope (subprogram) metadata ID
    current_scope: Option<usize>,
    /// Current source location (!DILocation ID), reused if line hasn't changed
    current_loc: Option<usize>,
    pub(crate) current_loc_line: usize,
    current_loc_file: String,
    /// Module flags and named metadata (emitted at very end)
    producer: String,
    /// Cache for basic type metadata IDs (avoids duplicates)
    type_cache: HashMap<String, usize>,
    /// Global variable expression IDs (for compile unit globals list)
    global_var_exprs: Vec<usize>,
}

impl DebugInfoBuilder {
    pub(crate) fn new(producer: &str) -> Self {
        Self {
            next_id: 0,
            metadata: Vec::new(),
            files: HashMap::new(),
            compile_unit: None,
            current_scope: None,
            current_loc: None,
            current_loc_line: 0,
            current_loc_file: String::new(),
            producer: producer.to_string(),
            type_cache: HashMap::new(),
            global_var_exprs: Vec::new(),
        }
    }

    fn alloc_id(&mut self) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Get or create a !DIFile node for the given filename.
    pub(crate) fn get_file(&mut self, filename: &str) -> usize {
        if let Some(&id) = self.files.get(filename) {
            return id;
        }
        let id = self.alloc_id();
        // Split into filename and directory
        let (dir, file) = if let Some(pos) = filename.rfind('/') {
            (&filename[..pos], &filename[pos + 1..])
        } else {
            (".", filename)
        };
        self.metadata.push(format!(
            "!{} = !DIFile(filename: \"{}\", directory: \"{}\")",
            id, escape_di_string(file), escape_di_string(dir)
        ));
        self.files.insert(filename.to_string(), id);
        id
    }

    /// Create the compile unit. Call once at the start.
    pub(crate) fn create_compile_unit(&mut self, filename: &str) -> usize {
        let file_id = self.get_file(filename);
        let id = self.alloc_id();
        // Use DW_LANG_C99 so lldb can inspect variables (lldb has no Modula-2
        // language plugin). M2 type names are preserved in DIBasicType metadata;
        // m2dap reformats the display to M2 conventions.
        self.metadata.push(format!(
            "!{} = distinct !DICompileUnit(language: DW_LANG_C99, file: !{}, \
             producer: \"{}\", isOptimized: false, runtimeVersion: 0, \
             emissionKind: FullDebug, splitDebugInlining: false)",
            id, file_id, escape_di_string(&self.producer)
        ));
        self.compile_unit = Some(id);
        id
    }

    /// Create a !DISubprogram for a function. Returns the metadata ID.
    /// The caller must attach `!dbg !N` to the `define` line.
    pub(crate) fn create_subprogram(
        &mut self,
        name: &str,
        linkage_name: &str,
        filename: &str,
        line: usize,
    ) -> usize {
        let file_id = self.get_file(filename);
        let cu_id = self.compile_unit.unwrap_or(0);

        // Create a subroutine type (void for now — type info is Phase 2)
        let sr_type_id = self.alloc_id();
        self.metadata.push(format!(
            "!{} = !DISubroutineType(types: !{{}})",
            sr_type_id
        ));

        let id = self.alloc_id();
        self.metadata.push(format!(
            "!{} = distinct !DISubprogram(name: \"{}\", linkageName: \"{}\", \
             scope: !{}, file: !{}, line: {}, type: !{}, \
             scopeLine: {}, spFlags: DISPFlagDefinition, unit: !{})",
            id,
            escape_di_string(name),
            escape_di_string(linkage_name),
            cu_id, file_id, line, sr_type_id, line, cu_id
        ));
        self.current_scope = Some(id);
        self.current_loc = None;
        self.current_loc_line = 0;
        id
    }

    /// Set the current debug location. Returns the metadata ID for !dbg.
    /// Returns None if the location hasn't changed (avoids duplicate metadata).
    pub(crate) fn set_location(&mut self, line: usize, col: usize, filename: &str) -> Option<usize> {
        if line == 0 {
            return None;
        }
        // Reuse existing location if line and file haven't changed
        if line == self.current_loc_line && filename == self.current_loc_file {
            return self.current_loc;
        }

        let scope = self.current_scope?;
        let id = self.alloc_id();
        self.metadata.push(format!(
            "!{} = !DILocation(line: {}, column: {}, scope: !{})",
            id, line, col, scope
        ));
        self.current_loc = Some(id);
        self.current_loc_line = line;
        self.current_loc_file = filename.to_string();
        Some(id)
    }

    /// Get the current debug location ID (if any).
    pub(crate) fn current_location(&self) -> Option<usize> {
        self.current_loc
    }

    // ── Phase 2: Type metadata and variable declarations ──────────

    /// Create a !DIBasicType for a primitive type. Returns metadata ID.
    pub(crate) fn create_basic_type(&mut self, name: &str, size_bits: usize, encoding: &str) -> usize {
        // Check cache
        let key = format!("{}:{}:{}", name, size_bits, encoding);
        if let Some(&id) = self.type_cache.get(&key) {
            return id;
        }
        let id = self.alloc_id();
        self.metadata.push(format!(
            "!{} = !DIBasicType(name: \"{}\", size: {}, encoding: {})",
            id, escape_di_string(name), size_bits, encoding
        ));
        self.type_cache.insert(key, id);
        id
    }

    /// Create a !DIDerivedType for a pointer type. Returns metadata ID.
    pub(crate) fn create_pointer_type(&mut self, base_type_id: usize, size_bits: usize) -> usize {
        let id = self.alloc_id();
        self.metadata.push(format!(
            "!{} = !DIDerivedType(tag: DW_TAG_pointer_type, baseType: !{}, size: {})",
            id, base_type_id, size_bits
        ));
        id
    }

    /// Create a !DICompositeType for an array. Returns metadata ID.
    pub(crate) fn create_array_type(&mut self, elem_type_id: usize, count: usize, elem_size_bits: usize) -> usize {
        let total_size = count * elem_size_bits;
        let subrange_id = self.alloc_id();
        self.metadata.push(format!(
            "!{} = !DISubrange(count: {})",
            subrange_id, count
        ));
        let id = self.alloc_id();
        self.metadata.push(format!(
            "!{} = !DICompositeType(tag: DW_TAG_array_type, baseType: !{}, size: {}, elements: !{{!{}}})",
            id, elem_type_id, total_size, subrange_id
        ));
        id
    }

    /// Create a !DICompositeType for a record (struct). Returns metadata ID.
    pub(crate) fn create_record_type(
        &mut self,
        name: &str,
        filename: &str,
        line: usize,
        size_bits: usize,
        members: Vec<(String, usize, usize, usize)>, // (name, type_id, size_bits, offset_bits)
    ) -> usize {
        // Check cache to avoid duplicate record types
        let cache_key = format!("record:{}", name);
        if let Some(&id) = self.type_cache.get(&cache_key) {
            return id;
        }
        let file_id = self.get_file(filename);
        // Pre-allocate the struct ID so members can reference it as scope
        let struct_id = self.alloc_id();
        // We'll fill in the struct metadata after members
        let struct_idx = self.metadata.len();
        self.metadata.push(String::new()); // placeholder

        let mut member_ids = Vec::new();
        for (mname, mtype, msize, moffset) in &members {
            let mid = self.alloc_id();
            self.metadata.push(format!(
                "!{} = !DIDerivedType(tag: DW_TAG_member, name: \"{}\", scope: !{}, \
                 file: !{}, line: {}, baseType: !{}, size: {}, offset: {})",
                mid, escape_di_string(mname), struct_id,
                file_id, line, mtype, msize, moffset
            ));
            member_ids.push(mid);
        }
        let members_str: Vec<String> = member_ids.iter().map(|id| format!("!{}", id)).collect();
        let elements_id = self.alloc_id();
        self.metadata.push(format!(
            "!{} = !{{{}}}",
            elements_id, members_str.join(", ")
        ));
        // Now fill in the struct metadata at the placeholder
        self.metadata[struct_idx] = format!(
            "!{} = !DICompositeType(tag: DW_TAG_structure_type, name: \"{}\", \
             file: !{}, line: {}, size: {}, elements: !{})",
            struct_id, escape_di_string(name), file_id, line, size_bits, elements_id
        );
        self.type_cache.insert(cache_key, struct_id);
        struct_id
    }

    /// Create a !DILocalVariable and return the metadata ID.
    /// The caller must emit `call void @llvm.dbg.declare(...)` with this ID.
    pub(crate) fn create_local_variable(
        &mut self,
        name: &str,
        filename: &str,
        line: usize,
        type_id: usize,
        arg_no: usize, // 0 for locals, 1+ for parameters
    ) -> usize {
        let file_id = self.get_file(filename);
        let scope = self.current_scope.unwrap_or(0);
        let id = self.alloc_id();
        if arg_no > 0 {
            self.metadata.push(format!(
                "!{} = !DILocalVariable(name: \"{}\", arg: {}, scope: !{}, \
                 file: !{}, line: {}, type: !{})",
                id, escape_di_string(name), arg_no, scope, file_id, line, type_id
            ));
        } else {
            self.metadata.push(format!(
                "!{} = !DILocalVariable(name: \"{}\", scope: !{}, \
                 file: !{}, line: {}, type: !{})",
                id, escape_di_string(name), scope, file_id, line, type_id
            ));
        }
        id
    }

    /// Create a !DIGlobalVariableExpression for a global variable.
    pub(crate) fn create_global_variable(
        &mut self,
        name: &str,
        linkage_name: &str,
        filename: &str,
        line: usize,
        type_id: usize,
    ) -> usize {
        let file_id = self.get_file(filename);
        let cu_id = self.compile_unit.unwrap_or(0);
        let var_id = self.alloc_id();
        self.metadata.push(format!(
            "!{} = !DIGlobalVariable(name: \"{}\", linkageName: \"{}\", \
             scope: !{}, file: !{}, line: {}, type: !{}, isLocal: false, isDefinition: true)",
            var_id, escape_di_string(name), escape_di_string(linkage_name),
            cu_id, file_id, line, type_id
        ));
        let expr_id = self.alloc_id();
        self.metadata.push(format!(
            "!{} = !DIGlobalVariableExpression(var: !{}, expr: !DIExpression())",
            expr_id, var_id
        ));
        self.global_var_exprs.push(expr_id);
        expr_id
    }

    /// Get the M2 type → DWARF type metadata ID for a basic M2 type name.
    pub(crate) fn get_m2_type(&mut self, type_name: &str) -> usize {
        match type_name {
            "INTEGER" => self.create_basic_type("INTEGER", 32, "DW_ATE_signed"),
            "CARDINAL" => self.create_basic_type("CARDINAL", 32, "DW_ATE_unsigned"),
            "LONGINT" => self.create_basic_type("LONGINT", 64, "DW_ATE_signed"),
            "LONGCARD" => self.create_basic_type("LONGCARD", 64, "DW_ATE_unsigned"),
            "REAL" => self.create_basic_type("REAL", 32, "DW_ATE_float"),
            "LONGREAL" => self.create_basic_type("LONGREAL", 64, "DW_ATE_float"),
            "BOOLEAN" => self.create_basic_type("BOOLEAN", 32, "DW_ATE_boolean"),
            "CHAR" => self.create_basic_type("CHAR", 8, "DW_ATE_unsigned_char"),
            "BITSET" => self.create_basic_type("BITSET", 32, "DW_ATE_unsigned"),
            "ADDRESS" => {
                let void_ty = self.create_basic_type("BYTE", 8, "DW_ATE_unsigned");
                self.create_pointer_type(void_ty, 64)
            }
            _ => self.create_basic_type(type_name, 32, "DW_ATE_signed"), // fallback
        }
    }

    /// Clear the current scope (when leaving a function).
    pub(crate) fn leave_scope(&mut self) {
        self.current_scope = None;
        self.current_loc = None;
        self.current_loc_line = 0;
    }

    /// Emit all accumulated metadata and module flags.
    /// Appended at the end of the .ll file.
    pub(crate) fn finalize(&self) -> String {
        if self.compile_unit.is_none() {
            return String::new();
        }
        let cu_id = self.compile_unit.unwrap();

        let mut out = String::new();
        out.push('\n');
        out.push_str("; Debug metadata\n");

        // Build globals list for compile unit (if any)
        let globals_suffix = if !self.global_var_exprs.is_empty() {
            let globals_list_id = self.next_id + 2; // after flag_id1 and flag_id2
            Some((globals_list_id, self.global_var_exprs.clone()))
        } else {
            None
        };

        // All metadata nodes, patching CU with globals if needed
        let cu_prefix = format!("!{} = distinct !DICompileUnit(", cu_id);
        for line in &self.metadata {
            if let Some((glist_id, _)) = &globals_suffix {
                if line.starts_with(&cu_prefix) {
                    // Patch CU to include globals reference
                    let patched = format!("{}, globals: !{}", line.trim_end_matches(')'), glist_id);
                    out.push_str(&patched);
                    out.push_str(")\n");
                    continue;
                }
            }
            out.push_str(line);
            out.push('\n');
        }

        // Module flags — use next IDs after all metadata
        let mut next = self.next_id;
        let flag_id1 = next; next += 1;
        let flag_id2 = next; next += 1;
        out.push_str(&format!(
            "!{} = !{{i32 2, !\"Debug Info Version\", i32 3}}\n", flag_id1));
        out.push_str(&format!(
            "!{} = !{{i32 7, !\"Dwarf Version\", i32 4}}\n", flag_id2));

        // Globals list (if any)
        if let Some((glist_id, ref exprs)) = globals_suffix {
            assert_eq!(glist_id, next);
            let refs: Vec<String> = exprs.iter().map(|id| format!("!{}", id)).collect();
            out.push_str(&format!("!{} = !{{{}}}\n", glist_id, refs.join(", ")));
            // next += 1; // not needed further
        }

        // Named metadata
        out.push_str(&format!("!llvm.dbg.cu = !{{!{}}}\n", cu_id));
        out.push_str(&format!("!llvm.module.flags = !{{!{}, !{}}}\n", flag_id1, flag_id2));

        out
    }

    /// Get the list of all subprogram metadata IDs (for attaching to `define` lines).
    pub(crate) fn current_scope_id(&self) -> Option<usize> {
        self.current_scope
    }
}

/// Escape special characters in debug info strings.
fn escape_di_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_debug_info() {
        let mut di = DebugInfoBuilder::new("mx 1.1.1");
        di.create_compile_unit("src/Main.mod");
        let sp = di.create_subprogram("Main", "Main_Main", "src/Main.mod", 5);
        assert!(sp > 0);
        let loc = di.set_location(10, 3, "src/Main.mod");
        assert!(loc.is_some());
        // Same location should return same ID
        let loc2 = di.set_location(10, 3, "src/Main.mod");
        assert_eq!(loc, loc2);
        // Different line should return new ID
        let loc3 = di.set_location(11, 3, "src/Main.mod");
        assert_ne!(loc, loc3);
    }

    #[test]
    fn test_finalize() {
        let mut di = DebugInfoBuilder::new("mx test");
        di.create_compile_unit("test.mod");
        di.create_subprogram("Test", "Test_Test", "test.mod", 1);
        di.set_location(5, 1, "test.mod");
        let output = di.finalize();
        assert!(output.contains("!llvm.dbg.cu"));
        assert!(output.contains("DICompileUnit"));
        assert!(output.contains("DISubprogram"));
        assert!(output.contains("DILocation"));
        assert!(output.contains("DW_LANG_C99"));
    }

    #[test]
    fn test_multiple_files() {
        let mut di = DebugInfoBuilder::new("mx test");
        di.create_compile_unit("main.mod");
        let f1 = di.get_file("main.mod");
        let f2 = di.get_file("utils.mod");
        assert_ne!(f1, f2);
        // Same file returns same ID
        let f3 = di.get_file("main.mod");
        assert_eq!(f1, f3);
    }
}
