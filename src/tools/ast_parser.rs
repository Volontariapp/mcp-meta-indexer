use std::fs;
use tree_sitter::{Parser, Point, Node};

pub struct ParsedContext {
    pub imports: String,
    pub target_block: String,
}

pub fn parse_file_context(filepath: &str, target_line_1_indexed: usize) -> Result<ParsedContext, String> {
    let source_code = fs::read_to_string(filepath).map_err(|e| e.to_string())?;
    
    let mut parser = Parser::new();
    let language = tree_sitter_typescript::language_typescript();
    parser.set_language(&language).map_err(|e| e.to_string())?;

    let tree = parser.parse(&source_code, None).ok_or("Failed to parse tree")?;
    let root_node = tree.root_node();

    let mut imports = String::new();
    let mut cursor = root_node.walk();
    for child in root_node.children(&mut cursor) {
        if child.kind() == "import_statement" {
            let import_text = &source_code[child.start_byte()..child.end_byte()];
            imports.push_str(import_text);
            imports.push('\n');
        }
    }

    let target_row = target_line_1_indexed.saturating_sub(1);
    let point = Point::new(target_row, 0);
    
    let mut current_node = root_node.named_descendant_for_point_range(point, point)
        .unwrap_or(root_node);

    let target_block = loop {
        let kind = current_node.kind();
        if kind == "class_declaration" 
            || kind == "method_definition" 
            || kind == "function_declaration" 
            || kind == "lexical_declaration"
            || kind == "export_statement" 
            || current_node.parent().is_none() {
            
            break &source_code[current_node.start_byte()..current_node.end_byte()];
        }
        
        if let Some(parent) = current_node.parent() {
            current_node = parent;
        } else {
            break &source_code[current_node.start_byte()..current_node.end_byte()];
        }
    };

    Ok(ParsedContext {
        imports,
        target_block: target_block.to_string(),
    })
}
