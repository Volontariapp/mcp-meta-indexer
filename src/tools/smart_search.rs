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

    // Execution de ripgrep dans le monorepo (ou le dossier spécifié)
    // On utilise un max de colonnes pour tronquer les très longues lignes compilées
    let output = Command::new("rg")
        .arg(query)
        .arg("--line-number")
        .arg("--max-columns=150")
        .current_dir(format!("../{}", directory))
        .output()
        .map_err(|e| format!("Erreur lors de l'exécution de ripgrep: {}", e))?;

    let mut result_text = String::from_utf8_lossy(&output.stdout).to_string();
    
    if result_text.trim().is_empty() {
        result_text = "Aucun résultat trouvé.".to_string();
    } else {
        // Tronquer le résultat s'il est trop long pour éviter d'exploser le contexte
        // RTK optimisera déjà cela, mais c'est une sécurité.
        if result_text.len() > 15000 {
            result_text.truncate(15000);
            result_text.push_str("\n... (résultats tronqués pour économiser les tokens)");
        }
    }

    Ok(CallToolResult {
        content: vec![ToolContent {
            content_type: "text".to_string(),
            text: result_text,
        }],
    })
}
