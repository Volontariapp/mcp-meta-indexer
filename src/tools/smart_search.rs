use std::process::Command;
use serde_json::{json, Value};
use crate::mcp_protocol::{Tool, ToolContent, CallToolResult};

pub fn get_tool_definition() -> Tool {
    Tool {
        name: "smart_search".to_string(),
        description: "Recherche intelligente dans la codebase (ripgrep optimisé) pour trouver des références et implémentations.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Texte ou regex à rechercher" },
                "directory": { "type": "string", "description": "Dossier cible optionnel (ex: 'ms-social' ou 'npm-packages')" }
            },
            "required": ["query"]
        }),
    }
}

pub fn execute(arguments: Value) -> Result<CallToolResult, String> {
    let query = arguments.get("query")
        .and_then(|v| v.as_str())
        .ok_or("Le paramètre 'query' est requis")?;

    let directory = arguments.get("directory")
        .and_then(|v| v.as_str())
        .unwrap_or(".");

    let base_path = format!("../{}", directory);
    
    // On demande à rg de renvoyer filepath:line_number:content
    let output = Command::new("rg")
        .arg(query)
        .arg("--line-number")
        .arg("--max-columns=150")
        .arg("--color=never")
        .current_dir(&base_path)
        .output()
        .map_err(|e| format!("Erreur lors de l'exécution de ripgrep: {}", e))?;

    let rg_output = String::from_utf8_lossy(&output.stdout);
    if rg_output.trim().is_empty() {
        return Ok(CallToolResult {
            content: vec![ToolContent {
                content_type: "text".to_string(),
                text: "Aucun résultat trouvé.".to_string(),
            }],
        });
    }

    let mut final_result = String::new();
    let mut files_processed = std::collections::HashSet::new();

    for line in rg_output.lines().take(20) { // Limiter aux 20 premiers matchs pour la perfs
        let parts: Vec<&str> = line.splitn(3, ':').collect();
        if parts.len() >= 2 {
            let filepath = parts[0];
            let line_number_str = parts[1];
            
            if let Ok(line_num) = line_number_str.parse::<usize>() {
                if !files_processed.insert(filepath.to_string()) {
                    continue; // On ne parse le fichier qu'une fois même s'il y a plusieurs matchs
                }

                let full_path = format!("{}/{}", base_path, filepath);
                
                final_result.push_str(&format!("\n=== Fichier: {} ===\n", filepath));
                
                // Si c'est un format supporté par notre AST Multi-langage
                if filepath.ends_with(".ts") || filepath.ends_with(".tsx") 
                   || filepath.ends_with(".rs") 
                   || filepath.ends_with(".json") 
                   || filepath.ends_with(".yaml") || filepath.ends_with(".yml") {
                    
                    if let Ok(context) = crate::tools::ast_parser::parse_file_context(&full_path, line_num) {
                        if !context.imports.is_empty() {
                            final_result.push_str("--- Imports (Contrats & Dépendances) ---\n");
                            final_result.push_str(&context.imports);
                        }
                        final_result.push_str("\n--- Contexte Sémantique (Minifié RTK) ---\n");
                        final_result.push_str(&context.target_block);
                        final_result.push('\n');
                    } else {  // Fallback
                        final_result.push_str(parts.get(2).unwrap_or(&""));
                    }
                } else {
                    // Fallback texte classique
                    final_result.push_str(parts.get(2).unwrap_or(&""));
                }
            }
        }
    }

    if final_result.len() > 20000 {
        final_result.truncate(20000);
        final_result.push_str("\n... (résultats tronqués)");
    }

    Ok(CallToolResult {
        content: vec![ToolContent {
            content_type: "text".to_string(),
            text: final_result,
        }],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_tool_definition() {
        let tool = get_tool_definition();
        assert_eq!(tool.name, "smart_search");
        assert!(tool.description.contains("ripgrep optimisé"));
        
        let schema = tool.input_schema;
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["required"][0], "query");
    }
}
