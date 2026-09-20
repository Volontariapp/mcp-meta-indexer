use std::sync::Arc;
use serde_json::{json, Value};
use crate::engine::doc_engine::query_docs;
use crate::engine::state::AppState;
use crate::mcp_protocol::{CallToolResult, Tool, ToolContent};

pub fn get_tool_definition() -> Tool {
    Tool {
        name: "search_docs".to_string(),
        description: "Recherche ciblée dans la documentation d'architecture C4 (meta/docs/ : C1-Context, C2-Containers, C3-Async, C4-Deployment, Monorepo-Structure). Extrait directement les sections conceptuelles pertinentes sans charger tout le fichier.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Concept architectural à rechercher (ex: 'Scatter-Gather', 'Neo4j', 'ArgoCD', 'Transactional Outbox', 'job_audit')"
                },
                "max_sections": {
                    "type": "integer",
                    "description": "Nombre maximum de sections à retourner (défaut: 3)"
                }
            },
            "required": ["query"]
        }),
    }
}

pub fn execute(state: &Arc<AppState>, arguments: Value) -> Result<CallToolResult, String> {
    let query = arguments
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or("Le paramètre 'query' est requis")?;

    let max_sections = arguments
        .get("max_sections")
        .and_then(|v| v.as_u64())
        .unwrap_or(3) as usize;

    let index = state.docs.read().unwrap();
    let result_text = query_docs(&index, query, max_sections, &state.root_dir);

    Ok(CallToolResult {
        content: vec![ToolContent {
            content_type: "text".to_string(),
            text: result_text,
        }],
    })
}
