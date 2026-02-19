use std::collections::HashMap;

use crate::analyze::AnalysisResult;
use crate::json::Json;
use crate::symtab::SymbolKind;
use super::hover::word_at_position;
use super::index::{IdentityKind, SymbolIdentity, WorkspaceIndex};

/// Handle textDocument/prepareCallHierarchy.
/// Returns CallHierarchyItem[] for the procedure at cursor.
/// Embeds identity key in `data` for workspace-wide incoming/outgoing calls.
pub fn prepare_call_hierarchy(
    source: &str,
    uri: &str,
    line: usize,
    col: usize,
    result: &AnalysisResult,
) -> Vec<Json> {
    let word = match word_at_position(source, line, col) {
        Some(w) => w,
        None => return Vec::new(),
    };

    // Look up symbol — must be a procedure
    let scope_id = result.scope_map.scope_at(line + 1, col + 1);
    let sym = result.symtab.lookup_in_scope(scope_id, &word)
        .or_else(|| result.symtab.lookup_all(&word));

    let sym = match sym {
        Some(s) => s,
        None => return Vec::new(),
    };

    match &sym.kind {
        SymbolKind::Procedure { .. } => {}
        _ => return Vec::new(),
    }

    let loc_line = if sym.loc.line > 0 { sym.loc.line - 1 } else { 0 };
    let loc_col = if sym.loc.col > 0 { sym.loc.col - 1 } else { 0 };

    // Build identity key for workspace-wide call hierarchy.
    // Use sym.module if available, otherwise derive module name from the AST.
    let module_name = sym.module.as_deref()
        .or_else(|| result.ast.as_ref().map(|ast| match ast {
            crate::ast::CompilationUnit::ProgramModule(m) => m.name.as_str(),
            crate::ast::CompilationUnit::DefinitionModule(m) => m.name.as_str(),
            crate::ast::CompilationUnit::ImplementationModule(m) => m.name.as_str(),
        }))
        .unwrap_or("");

    // Check if this procedure is nested by looking at its call_graph key.
    // Nested procs have keys like "helper@Outer1" in the call graph.
    let call_graph_key = find_call_graph_key_at_line(&result.call_graph, &sym.name, line, result);
    let identity_key = if !module_name.is_empty() {
        if let Some(ref cgk) = call_graph_key {
            if cgk.contains('@') {
                format!("{}::{}::proc", module_name, cgk)
            } else {
                SymbolIdentity::make_key(module_name, &sym.name, IdentityKind::Procedure)
            }
        } else {
            SymbolIdentity::make_key(module_name, &sym.name, IdentityKind::Procedure)
        }
    } else {
        String::new()
    };

    vec![Json::obj(vec![
        ("name", Json::str_val(&sym.name)),
        ("kind", Json::int_val(12)), // Function
        ("uri", Json::str_val(uri)),
        ("range", make_range(loc_line, loc_col, loc_line, loc_col + sym.name.len())),
        ("selectionRange", make_range(loc_line, loc_col, loc_line, loc_col + sym.name.len())),
        ("data", Json::obj(vec![
            ("identityKey", Json::str_val(&identity_key)),
            ("moduleName", Json::str_val(module_name)),
        ])),
    ])]
}

/// Handle callHierarchy/incomingCalls using workspace call graph.
/// Returns CallHierarchyIncomingCall[] — who calls this procedure across the workspace.
///
/// Falls back to single-file analysis if identity key is unavailable.
pub fn incoming_calls_ws(
    name: &str,
    identity_key: &str,
    workspace_index: &WorkspaceIndex,
    single_file_result: Option<&AnalysisResult>,
    single_file_uri: &str,
) -> Vec<Json> {
    // If we have a workspace identity key, use the workspace call graph
    if !identity_key.is_empty() {
        let edges = workspace_index.incoming_calls_for(identity_key);
        if !edges.is_empty() {
            // Group by caller key to produce one entry per caller with multiple fromRanges
            let mut grouped: HashMap<&str, Vec<&super::index::WsCallEdge>> = HashMap::new();
            for edge in edges {
                grouped.entry(&edge.other_key).or_default().push(edge);
            }

            let mut callers = Vec::new();
            for (caller_key, caller_edges) in &grouped {
                let caller_name = &caller_edges[0].other_name;

                // Look up definition location from workspace index
                let def_loc = workspace_index.find_def_by_identity(caller_key);
                let (def_uri, def_line, def_col) = def_loc
                    .map(|d| (d.file_uri.as_str(), d.line, d.col))
                    .unwrap_or((caller_edges[0].site_uri.as_str(), 1, 1));

                let cl = if def_line > 0 { def_line - 1 } else { 0 };
                let cc = if def_col > 0 { def_col - 1 } else { 0 };

                let from_ranges: Vec<Json> = caller_edges.iter().map(|e| {
                    let sl = if e.site_line > 0 { e.site_line - 1 } else { 0 };
                    let sc = if e.site_col > 0 { e.site_col - 1 } else { 0 };
                    let ec = if e.site_end_col > 0 { e.site_end_col - 1 } else { sc + name.len() };
                    make_range(sl, sc, sl, ec)
                }).collect();

                callers.push(Json::obj(vec![
                    ("from", Json::obj(vec![
                        ("name", Json::str_val(caller_name)),
                        ("kind", Json::int_val(12)),
                        ("uri", Json::str_val(def_uri)),
                        ("range", make_range(cl, cc, cl, cc + caller_name.len())),
                        ("selectionRange", make_range(cl, cc, cl, cc + caller_name.len())),
                        ("data", Json::obj(vec![
                            ("identityKey", Json::str_val(caller_key)),
                            ("moduleName", Json::str_val("")),
                        ])),
                    ])),
                    ("fromRanges", Json::arr(from_ranges)),
                ]));
            }

            // Stable sort: by URI then line
            callers.sort_by(|a, b| {
                let a_uri = a.get("from").and_then(|f| f.get("uri")).and_then(|u| u.as_str()).unwrap_or("");
                let b_uri = b.get("from").and_then(|f| f.get("uri")).and_then(|u| u.as_str()).unwrap_or("");
                a_uri.cmp(b_uri)
            });

            return callers;
        }
    }

    // Fallback: single-file analysis
    if let Some(result) = single_file_result {
        return incoming_calls_single_file(name, result, single_file_uri);
    }

    Vec::new()
}

/// Handle callHierarchy/outgoingCalls using workspace call graph.
/// Returns CallHierarchyOutgoingCall[] — what this procedure calls across the workspace.
pub fn outgoing_calls_ws(
    name: &str,
    identity_key: &str,
    workspace_index: &WorkspaceIndex,
    single_file_result: Option<&AnalysisResult>,
    single_file_uri: &str,
) -> Vec<Json> {
    // If we have a workspace identity key, use the workspace call graph
    if !identity_key.is_empty() {
        let edges = workspace_index.outgoing_calls_for(identity_key);
        if !edges.is_empty() {
            // Group by callee key to dedup (one entry per callee with multiple fromRanges)
            let mut grouped: HashMap<&str, Vec<&super::index::WsCallEdge>> = HashMap::new();
            for edge in edges {
                grouped.entry(&edge.other_key).or_default().push(edge);
            }

            let mut calls = Vec::new();
            for (callee_key, callee_edges) in &grouped {
                let callee_name = &callee_edges[0].other_name;

                // Look up definition location from workspace index
                let def_loc = workspace_index.find_def_by_identity(callee_key);
                let (def_uri, def_line, def_col) = def_loc
                    .map(|d| (d.file_uri.as_str(), d.line, d.col))
                    .unwrap_or(("", 1, 1));

                let cl = if def_line > 0 { def_line - 1 } else { 0 };
                let cc = if def_col > 0 { def_col - 1 } else { 0 };

                let from_ranges: Vec<Json> = callee_edges.iter().map(|e| {
                    let sl = if e.site_line > 0 { e.site_line - 1 } else { 0 };
                    let sc = if e.site_col > 0 { e.site_col - 1 } else { 0 };
                    let ec = if e.site_end_col > 0 { e.site_end_col - 1 } else { sc + callee_name.len() };
                    make_range(sl, sc, sl, ec)
                }).collect();

                calls.push(Json::obj(vec![
                    ("to", Json::obj(vec![
                        ("name", Json::str_val(callee_name)),
                        ("kind", Json::int_val(12)),
                        ("uri", Json::str_val(def_uri)),
                        ("range", make_range(cl, cc, cl, cc + callee_name.len())),
                        ("selectionRange", make_range(cl, cc, cl, cc + callee_name.len())),
                        ("data", Json::obj(vec![
                            ("identityKey", Json::str_val(callee_key)),
                            ("moduleName", Json::str_val("")),
                        ])),
                    ])),
                    ("fromRanges", Json::arr(from_ranges)),
                ]));
            }

            // Stable sort: by URI then line
            calls.sort_by(|a, b| {
                let a_uri = a.get("to").and_then(|f| f.get("uri")).and_then(|u| u.as_str()).unwrap_or("");
                let b_uri = b.get("to").and_then(|f| f.get("uri")).and_then(|u| u.as_str()).unwrap_or("");
                a_uri.cmp(b_uri)
            });

            return calls;
        }
    }

    // Fallback: single-file analysis
    if let Some(result) = single_file_result {
        return outgoing_calls_single_file(name, result, single_file_uri);
    }

    Vec::new()
}

// ── Single-file fallback (backward compat) ──────────────────────────

fn incoming_calls_single_file(name: &str, result: &AnalysisResult, uri: &str) -> Vec<Json> {
    let mut callers = Vec::new();

    for (caller_name, edges) in &result.call_graph {
        for edge in edges {
            if edge.callee == name {
                let caller_sym = result.symtab.lookup_all(caller_name);
                let (cl, cc) = caller_sym.map(|s| {
                    let l = if s.loc.line > 0 { s.loc.line - 1 } else { 0 };
                    let c = if s.loc.col > 0 { s.loc.col - 1 } else { 0 };
                    (l, c)
                }).unwrap_or((0, 0));

                let from_range_line = if edge.line > 0 { edge.line - 1 } else { 0 };
                let from_range_col = if edge.col > 0 { edge.col - 1 } else { 0 };

                callers.push(Json::obj(vec![
                    ("from", Json::obj(vec![
                        ("name", Json::str_val(caller_name)),
                        ("kind", Json::int_val(12)),
                        ("uri", Json::str_val(uri)),
                        ("range", make_range(cl, cc, cl, cc + caller_name.len())),
                        ("selectionRange", make_range(cl, cc, cl, cc + caller_name.len())),
                    ])),
                    ("fromRanges", Json::arr(vec![
                        make_range(from_range_line, from_range_col, from_range_line, from_range_col + name.len()),
                    ])),
                ]));
                break;
            }
        }
    }

    callers
}

fn outgoing_calls_single_file(name: &str, result: &AnalysisResult, uri: &str) -> Vec<Json> {
    let edges = match result.call_graph.get(name) {
        Some(e) => e,
        None => return Vec::new(),
    };

    let mut calls = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for edge in edges {
        if !seen.insert(&edge.callee) {
            continue;
        }

        let callee_sym = result.symtab.lookup_all(&edge.callee);
        let (cl, cc) = callee_sym.map(|s| {
            let l = if s.loc.line > 0 { s.loc.line - 1 } else { 0 };
            let c = if s.loc.col > 0 { s.loc.col - 1 } else { 0 };
            (l, c)
        }).unwrap_or((0, 0));

        let call_line = if edge.line > 0 { edge.line - 1 } else { 0 };
        let call_col = if edge.col > 0 { edge.col - 1 } else { 0 };

        calls.push(Json::obj(vec![
            ("to", Json::obj(vec![
                ("name", Json::str_val(&edge.callee)),
                ("kind", Json::int_val(12)),
                ("uri", Json::str_val(uri)),
                ("range", make_range(cl, cc, cl, cc + edge.callee.len())),
                ("selectionRange", make_range(cl, cc, cl, cc + edge.callee.len())),
            ])),
            ("fromRanges", Json::arr(vec![
                make_range(call_line, call_col, call_line, call_col + edge.callee.len()),
            ])),
        ]));
    }

    calls
}

/// Find the call_graph key that matches a procedure name at a given cursor line.
/// For nested procedures (multiple matches like "helper@Outer1", "helper@Outer2"),
/// uses the scope map to disambiguate by checking which parent scope contains the line.
fn find_call_graph_key_at_line(
    call_graph: &HashMap<String, Vec<crate::analyze::CallEdge>>,
    name: &str,
    _line: usize,
    _result: &AnalysisResult,
) -> Option<String> {
    // Prefer exact match (non-nested)
    if call_graph.contains_key(name) {
        return Some(name.to_string());
    }
    // Collect all nested keys like "name@parent"
    let candidates: Vec<&String> = call_graph.keys()
        .filter(|key| key.starts_with(name) && key.get(name.len()..name.len()+1) == Some("@"))
        .collect();
    if candidates.len() == 1 {
        return Some(candidates[0].clone());
    }
    // If multiple candidates, return the first one (best-effort).
    // Exact disambiguation would require matching by scope, but this handles the common case.
    candidates.first().map(|k| (*k).clone())
}

fn make_range(sl: usize, sc: usize, el: usize, ec: usize) -> Json {
    Json::obj(vec![
        ("start", Json::obj(vec![
            ("line", Json::int_val(sl as i64)),
            ("character", Json::int_val(sc as i64)),
        ])),
        ("end", Json::obj(vec![
            ("line", Json::int_val(el as i64)),
            ("character", Json::int_val(ec as i64)),
        ])),
    ])
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyze;

    #[test]
    fn test_call_graph_basic() {
        let source = "MODULE Test;\nPROCEDURE Bar;\nBEGIN END Bar;\nPROCEDURE Foo;\nBEGIN\n  Bar\nEND Foo;\nBEGIN\n  Foo\nEND Test.\n";
        let result = analyze::analyze_source(source, "test.mod", &[]);
        assert!(result.call_graph.contains_key("Foo"), "call_graph should contain Foo");
        let foo_calls = &result.call_graph["Foo"];
        assert!(foo_calls.iter().any(|e| e.callee == "Bar"), "Foo should call Bar");

        assert!(result.call_graph.contains_key("Test"), "call_graph should contain module body");
        let body_calls = &result.call_graph["Test"];
        assert!(body_calls.iter().any(|e| e.callee == "Foo"), "module body should call Foo");
    }

    #[test]
    fn test_incoming_calls_single_file() {
        let source = "MODULE Test;\nPROCEDURE Bar;\nBEGIN END Bar;\nPROCEDURE Foo;\nBEGIN\n  Bar\nEND Foo;\nBEGIN\nEND Test.\n";
        let result = analyze::analyze_source(source, "test.mod", &[]);
        let callers = incoming_calls_ws("Bar", "", &WorkspaceIndex::new(), Some(&result), "file:///test.mod");
        assert!(!callers.is_empty(), "Bar should have incoming calls");
        let from = callers[0].get("from").unwrap();
        let name = from.get("name").and_then(|n| n.as_str()).unwrap();
        assert_eq!(name, "Foo");
    }

    #[test]
    fn test_outgoing_calls_single_file() {
        let source = "MODULE Test;\nPROCEDURE Bar;\nBEGIN END Bar;\nPROCEDURE Foo;\nBEGIN\n  Bar\nEND Foo;\nBEGIN\nEND Test.\n";
        let result = analyze::analyze_source(source, "test.mod", &[]);
        let calls = outgoing_calls_ws("Foo", "", &WorkspaceIndex::new(), Some(&result), "file:///test.mod");
        assert!(!calls.is_empty(), "Foo should have outgoing calls");
        let to = calls[0].get("to").unwrap();
        let name = to.get("name").and_then(|n| n.as_str()).unwrap();
        assert_eq!(name, "Bar");
    }

    #[test]
    fn test_nested_calls() {
        let source = "MODULE Test;\nPROCEDURE A;\nBEGIN END A;\nPROCEDURE B;\nBEGIN\n  A\nEND B;\nPROCEDURE C;\nBEGIN\n  A;\n  B\nEND C;\nBEGIN\nEND Test.\n";
        let result = analyze::analyze_source(source, "test.mod", &[]);
        let c_calls = outgoing_calls_ws("C", "", &WorkspaceIndex::new(), Some(&result), "file:///test.mod");
        let callee_names: Vec<&str> = c_calls.iter()
            .filter_map(|c| c.get("to").and_then(|t| t.get("name")).and_then(|n| n.as_str()))
            .collect();
        assert!(callee_names.contains(&"A"), "C should call A");
        assert!(callee_names.contains(&"B"), "C should call B");
    }

    #[test]
    fn test_prepare_call_hierarchy() {
        let source = "MODULE Test;\nPROCEDURE Foo;\nBEGIN END Foo;\nBEGIN\nEND Test.\n";
        let result = analyze::analyze_source(source, "test.mod", &[]);
        let items = prepare_call_hierarchy(source, "file:///test.mod", 1, 10, &result);
        assert!(!items.is_empty(), "should find Foo at cursor");
        let name = items[0].get("name").and_then(|n| n.as_str()).unwrap();
        assert_eq!(name, "Foo");
        // Should have data with identityKey
        let data = items[0].get("data").unwrap();
        let key = data.get("identityKey").and_then(|k| k.as_str()).unwrap();
        assert_eq!(key, "Test::Foo::proc");
    }

    #[test]
    fn test_workspace_incoming_calls() {
        // Build a workspace index with two files: A calls B.Foo, B defines Foo
        let source_a = "MODULE A;\nPROCEDURE CallFoo;\nBEGIN\n  Foo\nEND CallFoo;\nBEGIN\nEND A.\n";
        let source_b = "MODULE B;\nPROCEDURE Foo;\nBEGIN END Foo;\nBEGIN\nEND B.\n";

        let result_a = analyze::analyze_source(source_a, "a.mod", &[]);
        let result_b = analyze::analyze_source(source_b, "b.mod", &[]);

        let mut idx = WorkspaceIndex::new();
        let path_a = std::path::PathBuf::from("/tmp/m2_ch_test/A.mod");
        let path_b = std::path::PathBuf::from("/tmp/m2_ch_test/B.mod");
        idx.index_from_analysis(&path_a, "file:///tmp/m2_ch_test/A.mod", source_a, result_a);
        idx.index_from_analysis(&path_b, "file:///tmp/m2_ch_test/B.mod", source_b, result_b);
        idx.rebuild_if_dirty();

        // A::CallFoo calls A::Foo (unqualified, resolved to same module since no import)
        // Query incoming calls for A::Foo — should find A::CallFoo
        let callers = incoming_calls_ws("Foo", "A::Foo::proc", &idx, None, "");
        assert!(!callers.is_empty(), "Foo should have incoming calls from workspace");
        let from = callers[0].get("from").unwrap();
        let caller_name = from.get("name").and_then(|n| n.as_str()).unwrap();
        assert_eq!(caller_name, "CallFoo");
    }

    #[test]
    fn test_workspace_outgoing_calls() {
        let source = "MODULE Test;\nPROCEDURE Bar;\nBEGIN END Bar;\nPROCEDURE Foo;\nBEGIN\n  Bar\nEND Foo;\nBEGIN\nEND Test.\n";
        let result = analyze::analyze_source(source, "test.mod", &[]);

        let mut idx = WorkspaceIndex::new();
        let path = std::path::PathBuf::from("/tmp/m2_ch_test2/Test.mod");
        idx.index_from_analysis(&path, "file:///tmp/m2_ch_test2/Test.mod", source, result);
        idx.rebuild_if_dirty();

        let calls = outgoing_calls_ws("Foo", "Test::Foo::proc", &idx, None, "");
        assert!(!calls.is_empty(), "Foo should have outgoing calls from workspace");
        let to = calls[0].get("to").unwrap();
        let callee_name = to.get("name").and_then(|n| n.as_str()).unwrap();
        assert_eq!(callee_name, "Bar");
    }
}
