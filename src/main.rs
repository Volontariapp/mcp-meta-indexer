mod mcp_protocol;
mod tools;

use axum::{
    routing::{get, post},
    Router, Json,
};
use serde_json::{json, Value};
use std::env;
use std::io::{self, BufRead, Write};
use mcp_protocol::{JsonRpcRequest, JsonRpcResponse, JsonRpcError};
use tower_http::cors::CorsLayer;

fn detect_workspace_root() -> String {
    if let Ok(root) = env::var("CODE_ROOT") {
        return root;
    }
    if let Ok(root) = env::var("WORKSPACE_ROOT") {
        return root;
    }
    if std::path::Path::new("/code").exists() {
        return "/code".to_string();
    }
    if std::path::Path::new("./npm-packages").exists() {
        return ".".to_string();
    }
    if std::path::Path::new("../npm-packages").exists() {
        return "..".to_string();
    }
    ".".to_string()
}

#[tokio::main]
async fn main() {
    let root = detect_workspace_root();
    let root_dep = root.clone();
    let root_impact = root.clone();

    // Lancement de l'indexation et du file watcher en arrière-plan
    std::thread::spawn(move || {
        tools::dependency_graph::start_indexer_and_watcher(root_dep);
    });
    std::thread::spawn(move || {
        tools::impact_graph::start_indexer_and_watcher(root_impact);
    });

    let transport = env::var("MCP_TRANSPORT").unwrap_or_else(|_| "stdio".to_string());

    if transport == "sse" {
        eprintln!("Démarrage du serveur MCP via SSE sur le port 3000...");
        let app = Router::new()
            .route("/sse", get(sse_handler))
            .route("/messages", post(messages_handler))
            .layer(CorsLayer::permissive());

        let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
        axum::serve(listener, app).await.unwrap();
    } else {
        // Boucle d'écoute Stdio
        let stdin = io::stdin();
        let mut handle = stdin.lock();
        let mut buffer = String::new();

        loop {
            buffer.clear();
            if handle.read_line(&mut buffer).unwrap() == 0 {
                break; // EOF
            }

            let trimmed = buffer.trim();
            if trimmed.is_empty() {
                continue;
            }

            match serde_json::from_str::<Value>(trimmed) {
                Ok(raw) => {
                    // Les notifications n'ont pas de "id" — on les traite sans répondre
                    let has_id = raw.get("id").is_some();
                    let method = raw.get("method").and_then(|m| m.as_str()).unwrap_or("");

                    // Notifications (pas de réponse attendue)
                    if !has_id && method.starts_with("notifications/") {
                        continue;
                    }

                    match serde_json::from_value::<JsonRpcRequest>(raw) {
                        Ok(request) => {
                            let response = handle_request(request).await;
                            let response_json = serde_json::to_string(&response).unwrap();
                            println!("{}", response_json);
                            io::stdout().flush().unwrap();
                        }
                        Err(e) => {
                            eprintln!("Erreur de parsing JsonRpcRequest: {}", e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Erreur de parsing JSON: {} | input: {}", e, trimmed);
                }
            }
        }
    }
}

async fn handle_request(req: JsonRpcRequest) -> JsonRpcResponse {
    let result = match req.method.as_str() {
        "initialize" => {
            let protocol_version = req.params.get("protocolVersion")
                .and_then(|v| v.as_str())
                .unwrap_or("2026-09-11");
                
            Ok(json!({
                "protocolVersion": protocol_version,
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "mcp-meta-indexer",
                    "version": "0.1.0"
                }
            }))
        }
        "notifications/initialized" => {
            Ok(json!({})) // Simple ack for the notification
        }
        "tools/list" => {
            let tool1 = tools::smart_search::get_tool_definition();
            let tool2 = tools::dependency_graph::get_tool_definition();
            let tool3 = tools::impact_graph::get_tool_definition();
            Ok(json!({ "tools": [tool1, tool2, tool3] }))
        }
        "tools/call" => {
            let name = req.params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let args = req.params.get("arguments").cloned().unwrap_or(json!({}));
            
            if name == "smart_search" {
                match tools::smart_search::execute(args) {
                    Ok(res) => Ok(serde_json::to_value(res).unwrap()),
                    Err(e) => Err(JsonRpcError { code: -32603, message: e }),
                }
            } else if name == "find_dependents" {
                match tools::dependency_graph::execute(args) {
                    Ok(res) => Ok(serde_json::to_value(res).unwrap()),
                    Err(e) => Err(JsonRpcError { code: -32603, message: e }),
                }
            } else if name == "analyze_impact" {
                match tools::impact_graph::execute(args) {
                    Ok(res) => Ok(serde_json::to_value(res).unwrap()),
                    Err(e) => Err(JsonRpcError { code: -32603, message: e }),
                }
            } else {
                Err(JsonRpcError { code: -32601, message: "Outil inconnu".to_string() })
            }
        }
        _ => Err(JsonRpcError { code: -32601, message: "Méthode non supportée".to_string() }),
    };

    match result {
        Ok(res) => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: req.id.unwrap_or(Value::Null),
            result: Some(res),
            error: None,
        },
        Err(err) => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: req.id.unwrap_or(Value::Null),
            result: None,
            error: Some(err),
        },
    }
}

// Handlers HTTP fictifs pour SSE (à compléter pour un vrai stream SSE)
async fn sse_handler() -> &'static str {
    "Endpoint SSE - En attente d'implémentation complète des events"
}

async fn messages_handler(Json(payload): Json<JsonRpcRequest>) -> Json<JsonRpcResponse> {
    let response = handle_request(payload).await;
    Json(response)
}
