use std::fs;
use tree_sitter::{Parser, Point};

pub struct ParsedContext {
    pub imports: String,
    pub target_block: String,
}

// Fonction de minification façon RTK
fn minify_rtk_style(code: &str) -> String {
    let mut minified = String::new();
    let mut prev_empty = false;
    
    for line in code.lines() {
        let trimmed = line.trim();
        
        // Supprimer les commentaires simples (on garde les JSDoc /** et RustDoc ///)
        if trimmed.starts_with("//") && !trimmed.starts_with("///") {
            continue;
        }
        
        // Supprimer les sauts de lignes multiples
        if trimmed.is_empty() {
            if !prev_empty {
                minified.push('\n');
                prev_empty = true;
            }
            continue;
        }
        
        prev_empty = false;
        
        // On garde une indentation minimale (1 espace par 4 espaces d'origine) 
        // ou on trim complètement pour économiser un max de tokens.
        // Faisons un trim complet (sauf si c'est collé, on laisse un espace pour la lisibilité).
        // Mais pour garder la structure, gardons juste le trimmed pour le moment.
        minified.push_str(trimmed);
        minified.push('\n');
    }
    
    minified
}

pub fn parse_file_context(filepath: &str, target_line_1_indexed: usize) -> Result<ParsedContext, String> {
    let source_code = fs::read_to_string(filepath).map_err(|e| e.to_string())?;
    
    let mut parser = Parser::new();
    
    let is_ts = filepath.ends_with(".ts") || filepath.ends_with(".tsx");
    let language = if filepath.ends_with(".rs") {
        tree_sitter_rust::LANGUAGE.into()
    } else if filepath.ends_with(".json") {
        tree_sitter_json::LANGUAGE.into()
    } else if filepath.ends_with(".yaml") || filepath.ends_with(".yml") {
        tree_sitter_yaml::LANGUAGE.into()
    } else {
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
    };
    
    parser.set_language(&language).map_err(|e| e.to_string())?;

    let tree = parser.parse(&source_code, None).ok_or("Failed to parse tree")?;
    let root_node = tree.root_node();

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

    // 2. Trouver le bloc parent englobant la ligne ciblée
    let target_row = target_line_1_indexed.saturating_sub(1);
    let point = Point::new(target_row, 0);
    
    let mut current_node = root_node.named_descendant_for_point_range(point, point)
        .unwrap_or(root_node);

    let target_block_raw = loop {
        let kind = current_node.kind();
        
        // TS & Rust blocks
        if kind == "class_declaration" 
            || kind == "method_definition" 
            || kind == "function_declaration" 
            || kind == "function_item" // Rust
            || kind == "impl_item" // Rust
            || kind == "struct_item" // Rust
            || kind == "lexical_declaration" 
            || kind == "export_statement" 
            || kind == "pair" // JSON / YAML key-value
            || current_node.parent().is_none() {
            
            break &source_code[current_node.start_byte()..current_node.end_byte()];
        }
        
        if let Some(parent) = current_node.parent() {
            current_node = parent;
        } else {
            break &source_code[current_node.start_byte()..current_node.end_byte()];
        }
    };

    // 3. Compression RTK
    let minified_block = minify_rtk_style(target_block_raw);

    Ok(ParsedContext {
        imports,
        target_block: minified_block,
    })
}
