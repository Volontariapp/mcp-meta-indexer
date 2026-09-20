mod domain;
mod engine;
mod infrastructure;
mod mcp_protocol;
mod tools;

use std::env;
use std::io::{self, BufRead, Write};
use std::sync::Arc;
use axum::{
    routing::{get, post},
    Extension, Json, Router,
};
use serde_json::{json, Value};
use tower_http::cors::CorsLayer;

use engine::dependency_engine::index_dependencies;
use engine::state::AppState;
use infrastructure::watcher::start_workspace_watcher;
use mcp_protocol::{JsonRpcError, JsonRpcRequest, JsonRpcResponse};

fn detect_workspace_root() -> String {
    if let Ok(root) = env::var("CODE_ROOT") {
        return root;
    }
    if let Ok(root) = env::var("WORKSPACE_ROOT") {
        return root;
    }

    // Détection automatique pour le cluster Kubernetes avec sidecar git-sync
    let k8s_candidates = [
        "/code/deploy.git/submodules",
        "/code/submodules",
        "/code/deploy/submodules",
    ];
    for path in k8s_candidates {
        if std::path::Path::new(&format!("{}/npm-packages", path)).exists() {
            return path.to_string();
        }
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
    eprintln!("🚀 Initialisation de mcp-meta-indexer sur la racine : '{}'", root);

    let state = Arc::new(AppState::new(root.clone()));

    // 1. Indexation asynchrone des dépendances au démarrage
    let state_deps = Arc::clone(&state);
    let root_deps = root.clone();
    std::thread::spawn(move || {
        index_dependencies(&state_deps, &root_deps);
        eprintln!("✅ Graphe de dépendances indexé en RAM.");
    });

    // 2. File watcher unifié
    let state_watcher = Arc::clone(&state);
    let root_watcher = root.clone();
    std::thread::spawn(move || {
        start_workspace_watcher(root_watcher, state_watcher);
    });

    let transport = env::var("MCP_TRANSPORT").unwrap_or_else(|_| "stdio".to_string());

    if transport == "sse" {
        eprintln!("Démarrage du serveur MCP via SSE sur le port 3000...");
        let app = Router::new()
            .route("/sse", get(sse_handler))
            .route("/messages", post(messages_handler))
            .layer(Extension(state))
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
                    let has_id = raw.get("id").is_some();
                    let method = raw.get("method").and_then(|m| m.as_str()).unwrap_or("");

                    if !has_id && method.starts_with("notifications/") {
                        continue;
                    }

                    match serde_json::from_value::<JsonRpcRequest>(raw) {
                        Ok(request) => {
                            let response = handle_request(&state, request).await;
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

async fn handle_request(state: &Arc<AppState>, req: JsonRpcRequest) -> JsonRpcResponse {
    let result = match req.method.as_str() {
        "initialize" => {
            let protocol_version = req
                .params
                .get("protocolVersion")
                .and_then(|v| v.as_str())
                .unwrap_or("2026-09-11");

            Ok(json!({
                "protocolVersion": protocol_version,
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "mcp-meta-indexer",
                    "version": "0.2.0"
                }
            }))
        }
        "notifications/initialized" => {
            Ok(json!({}))
        }
        "tools/list" => {
            let tool1 = tools::smart_search::get_tool_definition();
            let tool2 = tools::find_dependents::get_tool_definition();
            let tool3 = tools::analyze_impact::get_tool_definition();
            let tool4 = tools::analyze_grpc::get_tool_definition();
            let tool5 = tools::search_docs::get_tool_definition();
            Ok(json!({ "tools": [tool1, tool2, tool3, tool4, tool5] }))
        }
        "tools/call" => {
            let name = req.params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let args = req.params.get("arguments").cloned().unwrap_or(json!({}));

            match name {
                "smart_search" => match tools::smart_search::execute(state, args) {
                    Ok(res) => Ok(serde_json::to_value(res).unwrap()),
                    Err(e) => Err(JsonRpcError { code: -32603, message: e }),
                },
                "find_dependents" => match tools::find_dependents::execute(state, args) {
                    Ok(res) => Ok(serde_json::to_value(res).unwrap()),
                    Err(e) => Err(JsonRpcError { code: -32603, message: e }),
                },
                "analyze_impact" => match tools::analyze_impact::execute(state, args) {
                    Ok(res) => Ok(serde_json::to_value(res).unwrap()),
                    Err(e) => Err(JsonRpcError { code: -32603, message: e }),
                },
                "analyze_grpc" => match tools::analyze_grpc::execute(state, args) {
                    Ok(res) => Ok(serde_json::to_value(res).unwrap()),
                    Err(e) => Err(JsonRpcError { code: -32603, message: e }),
                },
                "search_docs" => match tools::search_docs::execute(state, args) {
                    Ok(res) => Ok(serde_json::to_value(res).unwrap()),
                    Err(e) => Err(JsonRpcError { code: -32603, message: e }),
                },
                _ => Err(JsonRpcError {
                    code: -32601,
                    message: format!("Outil inconnu: '{}'", name),
                }),
            }
        }
        _ => Err(JsonRpcError {
            code: -32601,
            message: "Méthode non supportée".to_string(),
        }),
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

async fn sse_handler() -> &'static str {
    "Endpoint SSE - mcp-meta-indexer v0.2.0"
}

async fn messages_handler(
    Extension(state): Extension<Arc<AppState>>,
    Json(payload): Json<JsonRpcRequest>,
) -> Json<JsonRpcResponse> {
    let response = handle_request(&state, payload).await;
    Json(response)
}
