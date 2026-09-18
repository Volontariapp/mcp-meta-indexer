use std::fs;
use tree_sitter::{Node, Parser, Point};

pub struct ParsedContext {
    pub imports: String,
    pub target_block: String,
    pub skeleton: String,
}

// Compression façon RTK
fn minify_rtk_style(code: &str) -> String {
    let mut minified = String::new();
    let mut prev_empty = false;

    for line in code.lines() {
        let trimmed = line.trim();

        // Supprimer les commentaires simples (on garde JSDoc /** et RustDoc ///)
        if trimmed.starts_with("//") && !trimmed.starts_with("///") {
            continue;
        }

        // Supprimer les sauts de lignes multiples consécutifs
        if trimmed.is_empty() {
            if !prev_empty {
                minified.push('\n');
                prev_empty = true;
            }
            continue;
        }

        prev_empty = false;
        minified.push_str(trimmed);
        minified.push('\n');
    }

    minified
}

/// Extrait la première ligne d'un nœud AST (= sa signature),
/// en retirant le `{` final si présent (corps de fonction/classe).
fn first_line_of(source_code: &str, start_byte: usize) -> String {
    let slice = &source_code[start_byte..];
    let first_line = slice.lines().next().unwrap_or("").trim();
    first_line.trim_end_matches('{').trim_end().to_string()
}

/// Extrait les signatures des membres directs d'un `class_body` (1 niveau).
/// Ignore le nœud qui contient `target_start_byte` (déjà dans target_block).
fn append_class_members(
    result: &mut String,
    class_node: Node,
    source_code: &str,
    target_start_byte: usize,
) {
    let mut cursor = class_node.walk();
    for child in class_node.children(&mut cursor) {
        if child.kind() != "class_body" {
            continue;
        }
        let mut body_cursor = child.walk();
        for member in child.children(&mut body_cursor) {
            if !member.is_named() {
                continue;
            }
            // Sauter le nœud cible (il est déjà dans target_block)
            if member.start_byte() <= target_start_byte && member.end_byte() > target_start_byte {
                continue;
            }
            if matches!(
                member.kind(),
                "method_definition" | "public_field_definition" | "property_definition"
            ) {
                let sig = first_line_of(source_code, member.start_byte());
                let line_num = member.start_position().row + 1;
                result.push_str(&format!("  {} // [L.{}]\n", sig, line_num));
            }
        }
        break; // Un seul class_body par classe
    }
}

/// Extrait les signatures des `function_item` dans un `impl_item` ou `trait_item` Rust (1 niveau).
fn append_impl_members(
    result: &mut String,
    impl_node: Node,
    source_code: &str,
    target_start_byte: usize,
) {
    let mut cursor = impl_node.walk();
    for child in impl_node.children(&mut cursor) {
        if child.kind() != "declaration_list" {
            continue;
        }
        let mut body_cursor = child.walk();
        for member in child.children(&mut body_cursor) {
            if !member.is_named() {
                continue;
            }
            if member.start_byte() <= target_start_byte && member.end_byte() > target_start_byte {
                continue;
            }
            if member.kind() == "function_item" {
                let sig = first_line_of(source_code, member.start_byte());
                let line_num = member.start_position().row + 1;
                result.push_str(&format!("  {} // [L.{}]\n", sig, line_num));
            }
        }
        break;
    }
}

/// Construit le squelette du fichier : signatures de toutes les déclarations top-level,
/// sauf le nœud contenant `target_start_byte` (déjà dans `target_block`).
///
/// Règles de profondeur :
/// - `class_declaration`  → 1 niveau (membres de la classe)
/// - `export_statement`   → 2 niveaux (unwrap export > class/function, puis membres si classe)
/// - `impl_item`          → 1 niveau (méthodes Rust)
/// - Tout le reste        → première ligne seulement
fn extract_skeleton(root_node: Node, source_code: &str, target_start_byte: usize) -> String {
    let mut skeleton = String::new();
    let mut cursor = root_node.walk();

    for child in root_node.children(&mut cursor) {
        if !child.is_named() {
            continue;
        }

        let contains_target =
            child.start_byte() <= target_start_byte && child.end_byte() > target_start_byte;
        let kind = child.kind();
        let line_num = child.start_position().row + 1;

        match kind {
            // ── TypeScript ──────────────────────────────────────────────────────
            "export_statement" => {
                if contains_target {
                    continue;
                }
                // Descendre dans l'export pour trouver la vraie déclaration (max 2 niveaux)
                let mut inner_cursor = child.walk();
                for inner in child.children(&mut inner_cursor) {
                    if !inner.is_named() {
                        continue;
                    }
                    let inner_kind = inner.kind();
                    let inner_line = inner.start_position().row + 1;
                    match inner_kind {
                        "class_declaration" => {
                            let sig = first_line_of(source_code, inner.start_byte());
                            skeleton.push_str(&format!("export {} {{\n", sig));
                            append_class_members(&mut skeleton, inner, source_code, target_start_byte);
                            skeleton.push_str("}\n");
                        }
                        "function_declaration" => {
                            let sig = first_line_of(source_code, inner.start_byte());
                            skeleton.push_str(&format!("export {} // [L.{}]\n", sig, inner_line));
                        }
                        "lexical_declaration" => {
                            // export const foo = ... → signature de l'export_statement entier
                            let sig = first_line_of(source_code, child.start_byte());
                            skeleton.push_str(&format!("{} // [L.{}]\n", sig, line_num));
                        }
                        _ => {}
                    }
                }
            }
            "class_declaration" => {
                if contains_target {
                    continue;
                }
                let sig = first_line_of(source_code, child.start_byte());
                skeleton.push_str(&format!("{} {{\n", sig));
                append_class_members(&mut skeleton, child, source_code, target_start_byte);
                skeleton.push_str("}\n");
            }
            "function_declaration" => {
                if contains_target {
                    continue;
                }
                let sig = first_line_of(source_code, child.start_byte());
                skeleton.push_str(&format!("{} // [L.{}]\n", sig, line_num));
            }
            "lexical_declaration" => {
                if contains_target {
                    continue;
                }
                let sig = first_line_of(source_code, child.start_byte());
                skeleton.push_str(&format!("{} // [L.{}]\n", sig, line_num));
            }
            // ── Rust ────────────────────────────────────────────────────────────
            "function_item" => {
                if contains_target {
                    continue;
                }
                let sig = first_line_of(source_code, child.start_byte());
                skeleton.push_str(&format!("{} // [L.{}]\n", sig, line_num));
            }
            "impl_item" | "trait_item" => {
                if contains_target {
                    continue;
                }
                let sig = first_line_of(source_code, child.start_byte());
                skeleton.push_str(&format!("{} {{\n", sig));
                append_impl_members(&mut skeleton, child, source_code, target_start_byte);
                skeleton.push_str("}\n");
            }
            "struct_item" | "enum_item" => {
                if contains_target {
                    continue;
                }
                let sig = first_line_of(source_code, child.start_byte());
                skeleton.push_str(&format!("{} // [L.{}]\n", sig, line_num));
            }
            _ => {}
        }
    }

    skeleton
}

pub fn parse_file_context(
    filepath: &str,
    target_line_1_indexed: usize,
) -> Result<ParsedContext, String> {
    let source_code = fs::read_to_string(filepath).map_err(|e| e.to_string())?;

    let mut parser = Parser::new();

    let is_ts = filepath.ends_with(".ts") || filepath.ends_with(".tsx");
    let is_rs = filepath.ends_with(".rs");

    let language = if is_rs {
        tree_sitter_rust::LANGUAGE.into()
    } else if filepath.ends_with(".json") {
        tree_sitter_json::LANGUAGE.into()
    } else if filepath.ends_with(".yaml") || filepath.ends_with(".yml") {
        tree_sitter_yaml::LANGUAGE.into()
    } else {
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
    };

    parser.set_language(&language).map_err(|e| e.to_string())?;

    let tree = parser
        .parse(&source_code, None)
        .ok_or("Failed to parse tree")?;
    let root_node = tree.root_node();

    // 1. Extraire les imports (TS uniquement)
    let mut imports = String::new();
    if is_ts {
        let mut cursor = root_node.walk();
        for child in root_node.children(&mut cursor) {
            if child.kind() == "import_statement" {
                let import_text = &source_code[child.start_byte()..child.end_byte()];
                imports.push_str(import_text);
                imports.push('\n');
            }
        }
    }

    // 2. Remonter l'AST depuis la ligne cible pour trouver le bloc parent englobant
    let target_row = target_line_1_indexed.saturating_sub(1);
    let point = Point::new(target_row, 0);

    let mut current_node = root_node
        .named_descendant_for_point_range(point, point)
        .unwrap_or(root_node);

    loop {
        let kind = current_node.kind();
        if kind == "class_declaration"
            || kind == "method_definition"
            || kind == "function_declaration"
            || kind == "function_item"   // Rust
            || kind == "impl_item"       // Rust
            || kind == "struct_item"     // Rust
            || kind == "lexical_declaration"
            || kind == "export_statement"
            || kind == "pair"            // JSON / YAML
            || current_node.parent().is_none()
        {
            break;
        }
        if let Some(parent) = current_node.parent() {
            current_node = parent;
        } else {
            break;
        }
    }

    let target_start_byte = current_node.start_byte();
    let target_block_raw = &source_code[target_start_byte..current_node.end_byte()];
    let minified_block = minify_rtk_style(target_block_raw);

    // 3. Extraire le squelette (TS + Rust uniquement — JSON/YAML non pertinents)
    let skeleton = if is_ts || is_rs {
        extract_skeleton(root_node, &source_code, target_start_byte)
    } else {
        String::new()
    };

    Ok(ParsedContext {
        imports,
        target_block: minified_block,
        skeleton,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minify_rtk_style() {
        let raw_code = "
function test() {
    // This is a normal comment that should be removed

    /// This is a rustdoc comment that must be kept
    let a = 1;


    let b = 2;
}
";
        let expected = "\nfunction test() {\n\n/// This is a rustdoc comment that must be kept\nlet a = 1;\n\nlet b = 2;\n}\n";
        let minified = minify_rtk_style(raw_code);
        assert_eq!(minified, expected);
    }

    #[test]
    fn test_first_line_of_removes_trailing_brace() {
        let code = "function foo(a: string): void {\n  // body\n}";
        let sig = first_line_of(code, 0);
        assert_eq!(sig, "function foo(a: string): void");
    }

    #[test]
    fn test_first_line_of_no_brace() {
        let code = "const foo = 42;\nconst bar = 43;";
        let sig = first_line_of(code, 0);
        assert_eq!(sig, "const foo = 42;");
    }

    #[test]
    fn test_extract_skeleton_typescript() {
        let source = "export function alpha(): void {\n  console.log('alpha');\n}\n\nexport function beta(): string {\n  return 'beta';\n}\n\nexport function gamma(x: number): number {\n  return x * 2;\n}\n";
        let tmp = std::env::temp_dir().join("test_skeleton_ts.ts");
        std::fs::write(&tmp, source).unwrap();

        // Cibler la ligne de "beta" (L.5)
        let ctx = parse_file_context(tmp.to_str().unwrap(), 5).unwrap();

        assert!(!ctx.skeleton.contains("beta"), "La cible ne doit pas être dans le skeleton");
        assert!(ctx.skeleton.contains("alpha"), "alpha doit être dans le skeleton");
        assert!(ctx.skeleton.contains("gamma"), "gamma doit être dans le skeleton");

        std::fs::remove_file(tmp).ok();
    }

    #[test]
    fn test_extract_skeleton_empty_for_json() {
        let source = r#"{"key": "value"}"#;
        let tmp = std::env::temp_dir().join("test_skeleton_json.json");
        std::fs::write(&tmp, source).unwrap();

        let ctx = parse_file_context(tmp.to_str().unwrap(), 1).unwrap();
        assert!(ctx.skeleton.is_empty(), "JSON ne doit pas générer de skeleton");

        std::fs::remove_file(tmp).ok();
    }
}
