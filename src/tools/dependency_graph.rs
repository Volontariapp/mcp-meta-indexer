use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;
use std::path::Path;
use serde_json::{json, Value};
use crate::mcp_protocol::{Tool, ToolContent, CallToolResult};
use once_cell::sync::Lazy;
use regex::Regex;
use ignore::WalkBuilder;
use notify::{Watcher, RecursiveMode, Event, EventKind};

// Le graphe en mémoire : clé = symbole ou package importé, valeur = ensemble de fichiers (chemins relatifs)
pub static DEPENDENCY_GRAPH: Lazy<Arc<RwLock<HashMap<String, HashSet<String>>>>> = 
    Lazy::new(|| Arc::new(RwLock::new(HashMap::new())));

pub fn get_tool_definition() -> Tool {
    Tool {
        name: "find_dependents".to_string(),
        description: "Recherche en O(1) dans le graphe de dépendances en mémoire pour trouver qui importe un symbole ou un package spécifique.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "target": { "type": "string", "description": "Nom du contrat (ex: UserAuthRequest) ou du package (ex: @volontariapp/domain-user)" }
            },
            "required": ["target"]
        }),
    }
}

pub fn execute(arguments: Value) -> Result<CallToolResult, String> {
    let target = arguments.get("target")
        .and_then(|v| v.as_str())
        .ok_or("Le paramètre 'target' est requis")?;

    let graph = DEPENDENCY_GRAPH.read().unwrap();
    
    if let Some(files) = graph.get(target) {
        let mut result_text = format!("Le symbole/package '{}' est importé dans {} fichier(s) :\n\n", target, files.len());
        for file in files {
            result_text.push_str(&format!("- {}\n", file));
        }
        
        Ok(CallToolResult {
            content: vec![ToolContent {
                content_type: "text".to_string(),
                text: result_text,
            }],
        })
    } else {
        Ok(CallToolResult {
            content: vec![ToolContent {
                content_type: "text".to_string(),
                text: format!("Aucune dépendance trouvée pour '{}' dans le graphe en mémoire.", target),
            }],
        })
    }
}

// Scanne un fichier et retourne les symboles et packages importés
fn extract_imports(content: &str) -> Vec<String> {
    let mut imports = Vec::new();
    
    // Regex simple pour `import { A, B } from 'pkg'` ou `import A from 'pkg'`
    let re = Regex::new(r#"(?m)^import\s+(?:\{([^}]+)\}|([a-zA-Z0-9_*]+))\s+from\s+['"]([^'"]+)['"]"#).unwrap();
    
    for cap in re.captures_iter(content) {
        // Extraction du package (cap 3)
        if let Some(pkg) = cap.get(3) {
            imports.push(pkg.as_str().to_string());
        }
        
        // Extraction des symboles (cap 1 destructuré)
        if let Some(symbols_block) = cap.get(1) {
            for symbol in symbols_block.as_str().split(',') {
                let s = symbol.trim();
                if !s.is_empty() {
                    // Gestion de `import { A as B }` -> on garde `A`
                    let parts: Vec<&str> = s.split(" as ").collect();
                    imports.push(parts[0].trim().to_string());
                }
            }
        }
        
        // Extraction du symbole par défaut (cap 2)
        if let Some(default_sym) = cap.get(2) {
            imports.push(default_sym.as_str().trim().to_string());
        }
    }
    
    imports
}

// Met à jour le graphe pour un fichier spécifique
fn update_file_in_graph(filepath: &str, content: &str) {
    let extracted = extract_imports(content);
    let mut graph = DEPENDENCY_GRAPH.write().unwrap();
    
    // Pour simplifier l'indexation dynamique, on pourrait nettoyer l'ancien index du fichier.
    // L'approche basique ici est d'ajouter le fichier aux nouveaux symboles sans nettoyage profond,
    // ce qui est suffisant pour le POC hot-reload de l'IA (et moins coûteux en locking).
    for import in extracted {
        graph.entry(import).or_insert_with(HashSet::new).insert(filepath.to_string());
    }
}

// Initialise le graphe complet en arrière-plan et lance le watcher
pub fn start_indexer_and_watcher(base_path: String) {
    thread::spawn(move || {
        println!("🚀 Démarrage de l'indexation du graphe de dépendances dans '{}'...", base_path);
        
        let start = std::time::Instant::now();
        let mut file_count = 0;
        
        // 1. Build initial complet
        let walker = WalkBuilder::new(&base_path)
            .hidden(false)
            .git_ignore(true)
            .build();
            
        for result in walker {
            if let Ok(entry) = result {
                if entry.file_type().map_or(false, |ft| ft.is_file()) {
                    let path_str = entry.path().to_string_lossy();
                    if path_str.ends_with(".ts") || path_str.ends_with(".tsx") {
                        if let Ok(content) = std::fs::read_to_string(entry.path()) {
                            update_file_in_graph(&path_str, &content);
                            file_count += 1;
                        }
                    }
                }
            }
        }
        
        println!("✅ Graphe construit en {:?} ! ({} fichiers indexés)", start.elapsed(), file_count);
        
        // 2. Lancement du watcher
        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = notify::recommended_watcher(tx).unwrap();
        
        if let Err(e) = watcher.watch(Path::new(&base_path), RecursiveMode::Recursive) {
            println!("⚠️ Impossible de lancer le file watcher sur {}: {}", base_path, e);
            return;
        }
        
        println!("👀 File watcher actif sur la codebase Volontariapp.");
        
        for res in rx {
            match res {
                Ok(Event { kind: EventKind::Modify(_), paths, .. }) |
                Ok(Event { kind: EventKind::Create(_), paths, .. }) => {
                    for path in paths {
                        let path_str = path.to_string_lossy();
                        if path_str.ends_with(".ts") || path_str.ends_with(".tsx") {
                            if let Ok(content) = std::fs::read_to_string(&path) {
                                update_file_in_graph(&path_str, &content);
                                println!("🔄 Graphe mis à jour suite à la modification de {}", path_str);
                            }
                        }
                    }
                },
                _ => {}
            }
        }
    });
}
