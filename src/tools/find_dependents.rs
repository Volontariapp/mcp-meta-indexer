use std::sync::Arc;
use serde_json::{json, Value};
use crate::engine::dependency_engine::query_dependents;
use crate::engine::state::AppState;
use crate::mcp_protocol::{CallToolResult, Tool, ToolContent};

pub fn get_tool_definition() -> Tool {
    Tool {
        name: "find_dependents".to_string(),
        description: "Recherche en O(1) dans le graphe de dépendances en mémoire pour trouver qui importe un symbole ou un package spécifique.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "target": {
                    "type": "string",
                    "description": "Nom du contrat (ex: UserAuthRequest) ou du package (ex: @volontariapp/domain-user)"
                }
            },
            "required": ["target"]
        }),
    }
}

pub fn execute(state: &Arc<AppState>, arguments: Value) -> Result<CallToolResult, String> {
    let target = arguments
        .get("target")
        .and_then(|v| v.as_str())
        .ok_or("Le paramètre 'target' est requis")?;

    let result_text = if let Some(files) = query_dependents(state, target) {
        let mut text = format!("Le symbole/package '{}' est importé dans {} fichier(s) :\n\n", target, files.len());
        for file in files {
            text.push_str(&format!("- {}\n", file));
        }

        if target.starts_with("@volontariapp")
            || target.contains("domain-")
            || target.contains("messaging")
            || target.contains("contracts")
        {
            text.push_str("\n════════════════════════════════════════════════════════════════════════════════\n");
            text.push_str("💡 GUIDANCE OPÉRATIONNELLE :\n");
            text.push_str("📦 Pour modifier ce package partagé : Skill `.agents/skills/global/shared-npm-package-change/SKILL.md`\n");
            text.push_str("🛑 RÈGLE DU STOP IMMÉDIAT : Après `yarn build` et `yarn changeset add`, STOP TOTAL !\n");
            text.push_str("   Interdiction formelle d'éditer les microservices consommateurs avant publication par la CI.\n");
            text.push_str("════════════════════════════════════════════════════════════════════════════════\n");
        }

        text
    } else {
        format!("Aucune dépendance trouvée pour '{}' dans le graphe en mémoire.", target)
    };

    Ok(CallToolResult {
        content: vec![ToolContent {
            content_type: "text".to_string(),
            text: result_text,
        }],
    })
}
