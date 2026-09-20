use std::process::Command;
use std::sync::Arc;
use serde_json::{json, Value};

use crate::engine::fuzzy_engine::FuzzyEngine;
use crate::engine::state::AppState;
use crate::infrastructure::ast::tree_sitter_parser::parse_file_context;
use crate::mcp_protocol::{CallToolResult, Tool, ToolContent};

pub fn get_tool_definition() -> Tool {
    Tool {
        name: "smart_search".to_string(),
        description: "Recherche intelligente dans la codebase (ripgrep optimisé + Tree-sitter AST) pour trouver des références et implémentations avec squelette architectural. Supporte le fallback fuzzy en cas de faute de frappe.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Texte ou regex à rechercher (ex: 'UserAuthRequest', 'createEvent')" },
                "scope": {
                    "type": "string",
                    "description": "Chemin absolu vers le repo ou dossier cible (OBLIGATOIRE). Ex: '/code/submodules/ms-social' ou '/code' pour tout le workspace."
                },
                "fuzzy": {
                    "type": "boolean",
                    "description": "Optionnel. Si true, active directement la recherche floue sur les symboles connus (défaut: false, activé automatiquement si aucun match exact)."
                }
            },
            "required": ["query", "scope"]
        }),
    }
}

pub fn execute(state: &Arc<AppState>, arguments: Value) -> Result<CallToolResult, String> {
    let query = arguments
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or("Le paramètre 'query' est requis")?;

    let scope = arguments
        .get("scope")
        .or_else(|| arguments.get("directory"))
        .and_then(|v| v.as_str())
        .unwrap_or(".");

    let force_fuzzy = arguments
        .get("fuzzy")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let base_path = if scope.starts_with('/') {
        scope.to_string()
    } else {
        format!("{}/{}", state.root_dir, scope.trim_start_matches("./"))
    };

    let rg_output = if !force_fuzzy {
        let output = Command::new("rg")
            .arg("--no-ignore")
            .arg("--iglob")
            .arg("**/*")
            .arg("--iglob")
            .arg("!**/node_modules/*/**")
            .arg("--iglob")
            .arg("**/node_modules/@volontariapp/**")
            .arg("--iglob")
            .arg("!**/.git/**")
            .arg(query)
            .arg("--line-number")
            .arg("--max-columns=150")
            .arg("--color=never")
            .current_dir(&base_path)
            .output()
            .map_err(|e| format!("Erreur lors de l'exécution de ripgrep: {}", e))?;

        String::from_utf8_lossy(&output.stdout).to_string()
    } else {
        String::new()
    };

    // Si ripgrep n'a rien trouvé ou si fuzzy est forcé -> Fallback Fuzzy Matching
    if rg_output.trim().is_empty() {
        let fuzzy_engine = FuzzyEngine::new();
        let mut candidates = Vec::new();

        // Collecter les symboles connus en mémoire
        {
            let deps = state.dependencies.read().unwrap();
            for k in deps.keys() {
                candidates.push(k.clone());
            }
        }
        {
            let async_flow = state.async_flow.read().unwrap();
            for k in async_flow.events.keys() {
                candidates.push(k.clone());
            }
            for k in async_flow.jobs.keys() {
                candidates.push(k.clone());
            }
        }
        {
            let grpc_flow = state.grpc_flow.read().unwrap();
            for svc in grpc_flow.services.values() {
                candidates.push(svc.service_name.clone());
                for m in svc.methods.keys() {
                    candidates.push(m.clone());
                }
            }
        }

        let best_matches = fuzzy_engine.find_best_matches(query, &candidates, 5);

        let mut fallback_text = format!(
            "Aucun résultat exact trouvé avec Ripgrep pour '{}' dans '{}'.\n",
            query, base_path
        );

        if !best_matches.is_empty() {
            fallback_text.push_str("\n💡 Suggestions lexicales les plus proches (Fuzzy Match) :\n");
            for (cand, score) in best_matches {
                fallback_text.push_str(&format!("  - `{}` (score: {})\n", cand, score));
            }
            fallback_text.push_str("\nTu peux relancer `smart_search`, `find_dependents` ou `analyze_impact` avec l'une de ces suggestions.");
        }

        return Ok(CallToolResult {
            content: vec![ToolContent {
                content_type: "text".to_string(),
                text: fallback_text,
            }],
        });
    }

    let mut final_result = String::new();
    let mut files_processed = std::collections::HashSet::new();

    for line in rg_output.lines().take(20) {
        let parts: Vec<&str> = line.splitn(3, ':').collect();
        if parts.len() >= 2 {
            let filepath = parts[0];
            let line_number_str = parts[1];

            if let Ok(line_num) = line_number_str.parse::<usize>() {
                if !files_processed.insert(filepath.to_string()) {
                    continue;
                }

                let full_path = format!("{}/{}", base_path, filepath);
                final_result.push_str(&format!("\n=== Fichier: {} ===\n", filepath));

                if filepath.ends_with(".ts")
                    || filepath.ends_with(".tsx")
                    || filepath.ends_with(".rs")
                    || filepath.ends_with(".json")
                    || filepath.ends_with(".yaml")
                    || filepath.ends_with(".yml")
                {
                    if let Ok(context) = parse_file_context(&full_path, line_num) {
                        if !context.imports.is_empty() {
                            final_result.push_str("--- Imports (Contrats & Dépendances) ---\n");
                            final_result.push_str(&context.imports);
                        }
                        final_result.push_str("\n--- Contexte Sémantique (Minifié RTK) ---\n");
                        final_result.push_str(&context.target_block);
                        final_result.push('\n');
                        if !context.skeleton.is_empty() {
                            final_result.push_str("\n--- Squelette du Fichier ---\n");
                            final_result.push_str(&context.skeleton);
                        }
                    } else {
                        final_result.push_str(parts.get(2).unwrap_or(&""));
                    }
                } else {
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
