use std::sync::Arc;
use serde_json::{json, Value};
use crate::engine::grpc_engine::query_grpc;
use crate::engine::state::AppState;
use crate::mcp_protocol::{CallToolResult, Tool, ToolContent};

pub fn get_tool_definition() -> Tool {
    Tool {
        name: "analyze_grpc".to_string(),
        description: "Cartographie et analyse synchrone des flux gRPC de bout en bout (définition .proto dans proto-registry, clients API Gateway/MS, et controllers microservices avec @GrpcMethod).".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "target": {
                    "type": "string",
                    "description": "Nom du service gRPC (ex: 'UserService'), de la méthode RPC (ex: 'SignUp', 'GetUser') ou du package."
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

    let graph = state.grpc_flow.read().unwrap();
    let result_text = query_grpc(&graph, target);

    Ok(CallToolResult {
        content: vec![ToolContent {
            content_type: "text".to_string(),
            text: result_text,
        }],
    })
}
