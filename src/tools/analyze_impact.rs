use std::sync::Arc;
use serde_json::{json, Value};
use crate::engine::impact_engine::query_impact;
use crate::engine::state::AppState;
use crate::mcp_protocol::{CallToolResult, Tool, ToolContent};

pub fn get_tool_definition() -> Tool {
    Tool {
        name: "analyze_impact".to_string(),
        description: "Cartographie et analyse l'impact architectural d'un flux asynchrone (CQRS, Outbox, BullMQ, Post-Processors, Sagas, WebSockets). Supporte la recherche bidirectionnelle (Amont/Aval) par nom d'événement, job, stream, queue ou classe de handler.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "target": {
                    "type": "string",
                    "description": "Nom de l'événement (ex: 'EVENT_CREATED', 'event.created'), du job (ex: 'PUBLISH_EVENT'), du stream ('stream:event-created') ou d'une classe (ex: 'EventCreatedPostProcessor', 'PublishEventHandler')."
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

    let graph = state.async_flow.read().unwrap();
    let result_text = query_impact(&graph, target);

    Ok(CallToolResult {
        content: vec![ToolContent {
            content_type: "text".to_string(),
            text: result_text,
        }],
    })
}
