use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Instant;
use ignore::WalkBuilder;
use notify::{Watcher, RecursiveMode, Event, EventKind};
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::{json, Value};

use crate::mcp_protocol::{Tool, ToolContent, CallToolResult};

// -----------------------------------------------------------------------------
// Modélisation des données en mémoire du Graphe Asynchrone
// -----------------------------------------------------------------------------

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct FlowEndpoint {
    pub service: String,
    pub file_path: String,
    pub line_number: usize,
    pub pattern: String,
    pub snippet: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct ConsumerEndpoint {
    pub service: String,
    pub handler_class: String,
    pub file_path: String,
    pub line_number: usize,
    pub processing_type: String, // "BatchPostProcessor" | "SinglePostProcessor" | "IJobHandler"
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct EventNode {
    pub event_key: String,              // ex: "EVENT_CREATED"
    pub raw_value: Option<String>,       // ex: "event.created"
    pub payload_interface: Option<String>,// ex: "IEventCreatedPayload"
    pub definition_file: Option<String>, // Fichier dans npm-packages/packages/messaging
    pub stream: Option<String>,          // ex: "stream:event-created"
    pub producers: Vec<FlowEndpoint>,    // Qui émet (domain-*, ms-*)
    pub consumers: Vec<ConsumerEndpoint>,// Qui écoute (post-processors, ws-service)
    pub saga_success: Option<String>,    // ex: "EVENT_CREATION_SUCCESSFULL"
    pub saga_failure: Option<String>,    // ex: "EVENT_CREATION_FAILED"
    pub websocket_event: Option<String>, // ex: "event.created.ws"
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct JobNode {
    pub job_key: String,                // ex: "PUBLISH_EVENT"
    pub raw_value: Option<String>,       // ex: "event.publish"
    pub payload_interface: Option<String>,// ex: "IPublishEventPayload"
    pub definition_file: Option<String>,
    pub queue: Option<String>,           // ex: "events-queue"
    pub producers: Vec<FlowEndpoint>,    // Qui crée le job (command controllers, withFallback)
    pub handlers: Vec<ConsumerEndpoint>, // Qui exécute (workers-runners)
}

#[derive(Debug, Clone, Default)]
pub struct AsyncFlowGraph {
    pub events: HashMap<String, EventNode>,
    pub jobs: HashMap<String, JobNode>,
    // Index inversé pour retrouver l'événement ou le job depuis un nom de classe handler ou post-processor
    pub consumer_to_target: HashMap<String, Vec<String>>,
}

// Instance globale protégée par RwLock
pub static IMPACT_GRAPH: Lazy<Arc<RwLock<AsyncFlowGraph>>> =
    Lazy::new(|| Arc::new(RwLock::new(AsyncFlowGraph::default())));

// -----------------------------------------------------------------------------
// Définition de l'outil MCP
// -----------------------------------------------------------------------------

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

// -----------------------------------------------------------------------------
// Résolution dynamique de la racine du workspace
// -----------------------------------------------------------------------------

pub fn resolve_workspace_root(hint: Option<&str>) -> String {
    if let Some(h) = hint {
        let trimmed = h.trim_end_matches('/');
        if Path::new(&format!("{}/npm-packages", trimmed)).exists() || Path::new(h).exists() {
            return trimmed.to_string();
        }
    }
    if let Ok(env_root) = std::env::var("WORKSPACE_ROOT") {
        let trimmed = env_root.trim_end_matches('/');
        if Path::new(trimmed).exists() {
            return trimmed.to_string();
        }
    }
    if Path::new("/code").exists() {
        return "/code".to_string();
    }
    if Path::new("../npm-packages").exists() {
        return "..".to_string();
    }
    if Path::new("npm-packages").exists() {
        return ".".to_string();
    }
    ".".to_string()
}

// -----------------------------------------------------------------------------
// Logique d'Indexation Multi-Passe
// -----------------------------------------------------------------------------

pub fn build_impact_graph(root_dir: &str) -> AsyncFlowGraph {
    let mut graph = AsyncFlowGraph::default();
    let root_clean = root_dir.trim_end_matches('/');

    // Regex pour détecter les définitions d'événements et de jobs dans messaging (const ou enum avec = ou :)
    let re_enum_entry = Regex::new(r#"(?m)^\s*([A-Z0-9_]+)\s*[:=]\s*['"]([^'"]+)['"]"#).unwrap();
    let re_registry_entry = Regex::new(r"(?m)\[([A-Za-z0-9_.]+)\]\s*:\s*([A-Za-z0-9_]+)").unwrap();
    
    // Regex pour détecter les émetteurs Outbox (EventQueueEntity.createEvent)
    let re_create_event = Regex::new(r"EventQueueEntity\.createEvent<([^>]+)>").unwrap();
    let re_create_event_simple = Regex::new(r"EventQueueEntity\.createEvent").unwrap();
    
    // Regex pour détecter les Jobs Outbox (JobsOutboxEntity.createJob ou withFallback)
    let re_create_job_simple = Regex::new(r"JobsOutboxEntity\.createJob|withFallback").unwrap();
    let re_job_target = Regex::new(r"JobsOutboxEntity\.createJob<([^>]+)>").unwrap();
    let re_fallback_target = Regex::new(r"withFallback\s*(?:<[^>]+>)?\s*\(\s*([A-Za-z0-9_.]+)").unwrap();

    // Regex pour détecter les consommateurs (Post-Processors et Workers)
    let re_post_processor = Regex::new(r"(?m)export\s+class\s+([A-Za-z0-9_]+)\s+extends\s+(BatchPostProcessor|SinglePostProcessor)<([^>]+)>").unwrap();
    let re_job_handler = Regex::new(r"(?m)export\s+class\s+([A-Za-z0-9_]+)\s+implements\s+IJobHandler<([^>]+)>").unwrap();
    let re_job_type_field = Regex::new(r"(?m)readonly\s+jobType\s*=\s*([A-Za-z0-9_.]+);").unwrap();

    // 1. Passe SSOT : Scan de npm-packages/packages/messaging
    let messaging_path = format!("{}/npm-packages/packages/messaging", root_clean);
    if Path::new(&messaging_path).exists() {
        let walker = WalkBuilder::new(&messaging_path)
            .hidden(false)
            .git_ignore(false)
            .build();

        for result in walker.flatten() {
            if result.file_type().map_or(false, |ft| ft.is_file()) {
                let path = result.path();
                let path_str = path.to_string_lossy();
                if path_str.ends_with(".ts") && !path_str.ends_with(".d.ts") {
                    if let Ok(content) = fs::read_to_string(path) {
                        // Extraction des enums d'événements
                        if path_str.contains("/events/") {
                            for cap in re_enum_entry.captures_iter(&content) {
                                let key = cap[1].to_string();
                                let val = cap[2].to_string();
                                let entry = graph.events.entry(key.clone()).or_insert_with(|| EventNode {
                                    event_key: key.clone(),
                                    ..Default::default()
                                });
                                entry.raw_value = Some(val);
                                entry.definition_file = Some(path_str.to_string());
                            }

                            // Extraction des mappings EventRegistry
                            for cap in re_registry_entry.captures_iter(&content) {
                                let full_key = cap[1].to_string();
                                let iface = cap[2].to_string();
                                let short_key = full_key.split('.').last().unwrap_or(&full_key);
                                if let Some(node) = graph.events.get_mut(short_key) {
                                    node.payload_interface = Some(iface);
                                }
                            }
                        }

                        // Extraction des enums de jobs
                        if path_str.contains("/jobs/") {
                            for cap in re_enum_entry.captures_iter(&content) {
                                let key = cap[1].to_string();
                                let val = cap[2].to_string();
                                let entry = graph.jobs.entry(key.clone()).or_insert_with(|| JobNode {
                                    job_key: key.clone(),
                                    ..Default::default()
                                });
                                entry.raw_value = Some(val);
                                entry.definition_file = Some(path_str.to_string());
                            }

                            // Extraction des mappings JobRegistry
                            for cap in re_registry_entry.captures_iter(&content) {
                                let full_key = cap[1].to_string();
                                let iface = cap[2].to_string();
                                let short_key = full_key.split('.').last().unwrap_or(&full_key);
                                if let Some(node) = graph.jobs.get_mut(short_key) {
                                    node.payload_interface = Some(iface);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. Passe globale : Scan de toute la codebase pour les Émetteurs et Consommateurs
    let mut overrides = ignore::overrides::OverrideBuilder::new(root_clean);
    let _ = overrides.add("**/*");
    let _ = overrides.add("!**/node_modules/**");
    let _ = overrides.add("!**/dist/**");
    let _ = overrides.add("!**/target/**");
    let _ = overrides.add("!**/.git/**");
    let override_set = overrides.build().unwrap_or_else(|_| ignore::overrides::Override::empty());

    let walker_all = WalkBuilder::new(root_clean)
        .hidden(false)
        .git_ignore(false)
        .overrides(override_set)
        .build();

    for result in walker_all.flatten() {
        if result.file_type().map_or(false, |ft| ft.is_file()) {
            let path = result.path();
            let path_str = path.to_string_lossy().to_string();
            if !path_str.ends_with(".ts") || path_str.ends_with(".d.ts") {
                continue;
            }

            let content = match fs::read_to_string(path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            // A. Détection des Émetteurs d'Événements
            if re_create_event_simple.is_match(&content) {
                for cap in re_create_event.captures_iter(&content) {
                    let generic_type = cap[1].trim();
                    let short_type = generic_type.split('.').last().unwrap_or(generic_type).to_string();
                    let match_start = cap.get(0).unwrap().start();
                    let match_end = cap.get(0).unwrap().end();
                    let line_num = content[..match_start].lines().count() + 1;

                    let slice_after = &content[match_end..content.len().min(match_end + 800)];

                    let mut stream_name = None;
                    if let Some(idx) = slice_after.find("Streams.") {
                        let stream_sub = &slice_after[idx..];
                        if let Some(end) = stream_sub.find(|c: char| !c.is_alphanumeric() && c != '_' && c != '.') {
                            stream_name = Some(stream_sub[..end].to_string());
                        }
                    }

                    let endpoint = FlowEndpoint {
                        service: extract_service_name(&path_str),
                        file_path: path_str.clone(),
                        line_number: line_num,
                        pattern: "EventQueueEntity.createEvent".to_string(),
                        snippet: slice_after.chars().take(180).collect(),
                    };

                    let entry = graph.events.entry(short_type.clone()).or_insert_with(|| EventNode {
                        event_key: short_type.clone(),
                        ..Default::default()
                    });
                    
                    if stream_name.is_some() {
                        entry.stream = stream_name;
                    }

                    entry.producers.push(endpoint);
                }
            }

            // B. Détection des Émetteurs de Jobs (createJob et withFallback)
            if re_create_job_simple.is_match(&content) {
                // createJob
                for cap in re_job_target.captures_iter(&content) {
                    let full_type = cap[1].trim();
                    let clean_type = full_type.replace("typeof", "").trim().to_string();
                    let short_type = clean_type.split('.').last().unwrap_or(&clean_type).to_string();
                    let line_num = content[..cap.get(0).unwrap().start()].lines().count();

                    let endpoint = FlowEndpoint {
                        service: extract_service_name(&path_str),
                        file_path: path_str.clone(),
                        line_number: line_num,
                        pattern: "JobsOutboxEntity.createJob".to_string(),
                        snippet: cap[0].chars().take(150).collect(),
                    };

                    let entry = graph.jobs.entry(short_type.clone()).or_insert_with(|| JobNode {
                        job_key: short_type.clone(),
                        ..Default::default()
                    });
                    entry.producers.push(endpoint);
                }

                // withFallback
                for cap in re_fallback_target.captures_iter(&content) {
                    let full_type = cap[1].trim();
                    let short_type = full_type.split('.').last().unwrap_or(full_type).to_string();
                    let line_num = content[..cap.get(0).unwrap().start()].lines().count();

                    let endpoint = FlowEndpoint {
                        service: extract_service_name(&path_str),
                        file_path: path_str.clone(),
                        line_number: line_num,
                        pattern: "BaseCommandController.withFallback".to_string(),
                        snippet: cap[0].chars().take(150).collect(),
                    };

                    let entry = graph.jobs.entry(short_type.clone()).or_insert_with(|| JobNode {
                        job_key: short_type.clone(),
                        ..Default::default()
                    });
                    entry.producers.push(endpoint);
                }
            }

            // C. Détection des Consommateurs d'Événements (Post-Processors)
            for cap in re_post_processor.captures_iter(&content) {
                let class_name = cap[1].to_string();
                let proc_type = cap[2].to_string();
                let full_event_type = cap[3].trim().to_string();
                let short_event = full_event_type.split('.').last().unwrap_or(&full_event_type).to_string();
                let line_num = content[..cap.get(0).unwrap().start()].lines().count();

                let consumer = ConsumerEndpoint {
                    service: extract_service_name(&path_str),
                    handler_class: class_name.clone(),
                    file_path: path_str.clone(),
                    line_number: line_num,
                    processing_type: proc_type,
                };

                let entry = graph.events.entry(short_event.clone()).or_insert_with(|| EventNode {
                    event_key: short_event.clone(),
                    ..Default::default()
                });
                entry.consumers.push(consumer);

                // Enregistrement dans l'index inversé
                graph.consumer_to_target.entry(class_name.clone())
                    .or_default()
                    .push(short_event.clone());
            }

            // D. Détection des Consommateurs de Jobs (Workers & Handlers)
            if let Some(cap) = re_job_handler.captures(&content) {
                let class_name = cap[1].to_string();
                let mut target_job = cap[2].replace("typeof", "").trim().to_string();
                if let Some(short) = target_job.split('.').last() {
                    target_job = short.to_string();
                }

                // Si le jobType est redéfini dans le corps (ex: `readonly jobType = JobMessagingType.PUBLISH_EVENT`)
                if let Some(fcap) = re_job_type_field.captures(&content) {
                    let field_val = fcap[1].trim();
                    if let Some(short) = field_val.split('.').last() {
                        target_job = short.to_string();
                    }
                }

                let line_num = content[..cap.get(0).unwrap().start()].lines().count();
                let consumer = ConsumerEndpoint {
                    service: extract_service_name(&path_str),
                    handler_class: class_name.clone(),
                    file_path: path_str.clone(),
                    line_number: line_num,
                    processing_type: "IJobHandler".to_string(),
                };

                let entry = graph.jobs.entry(target_job.clone()).or_insert_with(|| JobNode {
                    job_key: target_job.clone(),
                    ..Default::default()
                });
                entry.handlers.push(consumer);

                // Index inversé
                graph.consumer_to_target.entry(class_name.clone())
                    .or_default()
                    .push(target_job.clone());
            }
        }
    }

    // 3. Liaison automatique des Sagas (Triade d'événements)
    let event_keys: Vec<String> = graph.events.keys().cloned().collect();
    for key in &event_keys {
        if key.ends_with("_CREATED") {
            let base = &key[..key.len() - 8];
            let success_candidate = format!("{}_CREATION_SUCCESSFULL", base);
            let failure_candidate = format!("{}_CREATION_FAILED", base);

            if graph.events.contains_key(&success_candidate) {
                if let Some(node) = graph.events.get_mut(key) {
                    node.saga_success = Some(success_candidate);
                }
            }
            if graph.events.contains_key(&failure_candidate) {
                if let Some(node) = graph.events.get_mut(key) {
                    node.saga_failure = Some(failure_candidate);
                }
            }
        } else if key.ends_with("_DELETED") {
            let base = &key[..key.len() - 8];
            let success_candidate = format!("{}_DELETION_SUCCESSFULL", base);
            let failure_candidate = format!("{}_DELETION_FAILED", base);

            if graph.events.contains_key(&success_candidate) {
                if let Some(node) = graph.events.get_mut(key) {
                    node.saga_success = Some(success_candidate);
                }
            }
            if graph.events.contains_key(&failure_candidate) {
                if let Some(node) = graph.events.get_mut(key) {
                    node.saga_failure = Some(failure_candidate);
                }
            }
        }
    }

    graph
}

// Déduit le microservice ou runner depuis le chemin de fichier
fn extract_service_name(path: &str) -> String {
    for part in path.split('/') {
        if part.starts_with("ms-") 
            || part.starts_with("post-processor-") 
            || part.starts_with("worker-") 
            || part.starts_with("outbox-") 
            || part.starts_with("domain-") 
            || part == "ws-service" 
            || part == "api-gateway" {
            return part.to_string();
        }
    }
    "unknown".to_string()
}

// -----------------------------------------------------------------------------
// Démarrage de l'indexation et du File Watcher
// -----------------------------------------------------------------------------

pub fn start_indexer_and_watcher(root_dir: String) {
    let resolved_root = resolve_workspace_root(Some(&root_dir));
    eprintln!("🚀 Initialisation du Graphe d'Impact Asynchrone dans '{}'...", resolved_root);
    let start = Instant::now();

    let graph = build_impact_graph(&resolved_root);
    let event_count = graph.events.len();
    let job_count = graph.jobs.len();

    {
        let mut global = IMPACT_GRAPH.write().unwrap();
        *global = graph;
    }

    eprintln!(
        "✅ Graphe d'Impact construit en {:?} ! ({} événements, {} jobs indexés)",
        start.elapsed(),
        event_count,
        job_count
    );

    // Watcher notify en arrière-plan
    let root_clone = resolved_root.clone();
    thread::spawn(move || {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = match notify::recommended_watcher(tx) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("Erreur lors de l'initialisation du watcher notify: {}", e);
                return;
            }
        };

        let watch_path = Path::new(&root_clone);
        if let Err(e) = watcher.watch(watch_path, RecursiveMode::Recursive) {
            eprintln!("Impossible de surveiller '{}': {}", root_clone, e);
            return;
        }

        for res in rx {
            if let Ok(Event { kind, paths, .. }) = res {
                match kind {
                    EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_) => {
                        let should_reindex = paths.iter().any(|p| {
                            let s = p.to_string_lossy();
                            s.ends_with(".ts") && !s.contains("/dist/") && !s.contains("/node_modules/")
                        });

                        if should_reindex {
                            let updated = build_impact_graph(&root_clone);
                            let mut global = IMPACT_GRAPH.write().unwrap();
                            *global = updated;
                        }
                    }
                    _ => {}
                }
            }
        }
    });
}

// -----------------------------------------------------------------------------
// Exécution de l'outil MCP `analyze_impact`
// -----------------------------------------------------------------------------

pub fn execute(arguments: Value) -> Result<CallToolResult, String> {
    let target = arguments.get("target")
        .and_then(|v| v.as_str())
        .ok_or("Le paramètre 'target' est requis")?
        .trim();

    let graph = IMPACT_GRAPH.read().unwrap();
    let target_upper = target.to_uppercase();
    let target_lower = target.to_lowercase();

    let mut found = false;
    let mut out = String::new();

    // 1. Recherche par Événement
    for (key, event) in &graph.events {
        let matches_key = key.to_uppercase().contains(&target_upper);
        let matches_raw = event.raw_value.as_ref().map_or(false, |r| r.to_lowercase().contains(&target_lower));

        if matches_key || matches_raw {
            found = true;
            out.push_str(&format!("================================================================================\n"));
            out.push_str(&format!("⚡ ÉVÉNEMENT DISTRIBUÉ : {}\n", key));
            if let Some(raw) = &event.raw_value {
                out.push_str(&format!("   Valeur Bus : '{}'\n", raw));
            }
            out.push_str(&format!("================================================================================\n"));

            out.push_str(&format!("📜 Contrat & Payload :\n"));
            if let Some(iface) = &event.payload_interface {
                out.push_str(&format!("   - Interface Payload : {}\n", iface));
            }
            if let Some(file) = &event.definition_file {
                out.push_str(&format!("   - Définition : {}\n", file));
            }
            if let Some(stream) = &event.stream {
                out.push_str(&format!("   - Stream Redis : {}\n", stream));
            }

            out.push_str(&format!("\n🚀 Émetteur(s) (Amont / Producers) : {}\n", event.producers.len()));
            if event.producers.is_empty() {
                out.push_str("   (Aucun émetteur direct détecté via EventQueueEntity)\n");
            } else {
                for (i, p) in event.producers.iter().enumerate() {
                    out.push_str(&format!("   {}. [{}] {}\n", i + 1, p.service, p.pattern));
                    out.push_str(&format!("      Fichier : {}:{}\n", p.file_path, p.line_number));
                }
            }

            out.push_str(&format!("\n⚡ Consommateur(s) (Aval / Consumers) : {}\n", event.consumers.len()));
            if event.consumers.is_empty() {
                out.push_str("   (Aucun post-processor ou listener direct détecté)\n");
            } else {
                for (i, c) in event.consumers.iter().enumerate() {
                    out.push_str(&format!("   {}. [{}] {} ({})\n", i + 1, c.service, c.handler_class, c.processing_type));
                    out.push_str(&format!("      Fichier : {}:{}\n", c.file_path, c.line_number));
                }
            }

            // Sagas & Rollback
            if event.saga_success.is_some() || event.saga_failure.is_some() {
                out.push_str(&format!("\n🔄 Triade de Saga (Chorégraphie & Compensations) :\n"));
                if let Some(succ) = &event.saga_success {
                    out.push_str(&format!("   - Succès (Commit -> SagaStatus.DONE)   : {}\n", succ));
                }
                if let Some(fail) = &event.saga_failure {
                    out.push_str(&format!("   - Échec  (Rollback -> SagaStatus.CANCEL) : {}\n", fail));
                }
            }
            out.push('\n');
        }
    }

    // 2. Recherche par Job
    for (key, job) in &graph.jobs {
        let matches_key = key.to_uppercase().contains(&target_upper);
        let matches_raw = job.raw_value.as_ref().map_or(false, |r| r.to_lowercase().contains(&target_lower));

        if matches_key || matches_raw {
            found = true;
            out.push_str(&format!("================================================================================\n"));
            out.push_str(&format!("🛠️ JOB D'ARRIÈRE-PLAN (1:1) : {}\n", key));
            if let Some(raw) = &job.raw_value {
                out.push_str(&format!("   Valeur Job : '{}'\n", raw));
            }
            out.push_str(&format!("================================================================================\n"));

            out.push_str(&format!("📜 Contrat & Payload :\n"));
            if let Some(iface) = &job.payload_interface {
                out.push_str(&format!("   - Interface Payload : {}\n", iface));
            }
            if let Some(file) = &job.definition_file {
                out.push_str(&format!("   - Définition : {}\n", file));
            }
            if let Some(q) = &job.queue {
                out.push_str(&format!("   - Queue BullMQ : {}\n", q));
            }

            out.push_str(&format!("\n🚀 Émetteur(s) (Amont / Producers) : {}\n", job.producers.len()));
            if job.producers.is_empty() {
                out.push_str("   (Aucun émetteur direct détecté via JobsOutboxEntity)\n");
            } else {
                for (i, p) in job.producers.iter().enumerate() {
                    out.push_str(&format!("   {}. [{}] {}\n", i + 1, p.service, p.pattern));
                    out.push_str(&format!("      Fichier : {}:{}\n", p.file_path, p.line_number));
                }
            }

            out.push_str(&format!("\n⚙️ Exécuteur(s) (Workers / Handlers) : {}\n", job.handlers.len()));
            if job.handlers.is_empty() {
                out.push_str("   (Aucun IJobHandler détecté)\n");
            } else {
                for (i, h) in job.handlers.iter().enumerate() {
                    out.push_str(&format!("   {}. [{}] {}\n", i + 1, h.service, h.handler_class));
                    out.push_str(&format!("      Fichier : {}:{}\n", h.file_path, h.line_number));
                }
            }
            out.push('\n');
        }
    }

    // 3. Recherche Inversée : Si la cible est une classe de Handler ou Post-Processor
    for (consumer_name, targets) in &graph.consumer_to_target {
        if consumer_name.to_lowercase().contains(&target_lower) {
            found = true;
            out.push_str(&format!("================================================================================\n"));
            out.push_str(&format!("🔍 NAVIGATION INVERSÉE (Upstream Trace) pour '{}'\n", consumer_name));
            out.push_str(&format!("================================================================================\n"));
            out.push_str(&format!("Cette classe consomme les événements / jobs suivants :\n"));
            let mut unique_targets: Vec<&String> = targets.iter().collect();
            unique_targets.sort();
            unique_targets.dedup();
            for t in unique_targets {
                out.push_str(&format!("- {}\n", t));
            }
            out.push('\n');
        }
    }

    if !found {
        return Ok(CallToolResult {
            content: vec![ToolContent {
                content_type: "text".to_string(),
                text: format!(
                    "Aucun événement, job, ou handler correspondant à '{}' n'a été trouvé dans le graphe asynchrone.",
                    target
                ),
            }],
        });
    }

    Ok(CallToolResult {
        content: vec![ToolContent {
            content_type: "text".to_string(),
            text: out,
        }],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_definition() {
        let tool = get_tool_definition();
        assert_eq!(tool.name, "analyze_impact");
        assert!(tool.description.contains("CQRS"));
    }

    #[test]
    fn test_extract_service_name() {
        assert_eq!(extract_service_name("/code/ms-event/src/main.ts"), "ms-event");
        assert_eq!(extract_service_name("../post-processors-runner/post-processor-social/src/index.ts"), "post-processor-social");
        assert_eq!(extract_service_name("npm-packages/packages/domain-user/src/repo.ts"), "domain-user");
    }

    #[test]
    fn test_real_impact_graph_indexing() {
        let root = resolve_workspace_root(Some("../"));
        let graph = build_impact_graph(&root);
        eprintln!("Indexed {} events, {} jobs", graph.events.len(), graph.jobs.len());
        assert!(!graph.events.is_empty(), "Doit indexer au moins un événement depuis messaging");
        assert!(graph.events.contains_key("EVENT_CREATED"), "Doit contenir EVENT_CREATED");
        let event_created = &graph.events["EVENT_CREATED"];
        assert_eq!(event_created.saga_success.as_deref(), Some("EVENT_CREATION_SUCCESSFULL"));
        assert_eq!(event_created.saga_failure.as_deref(), Some("EVENT_CREATION_FAILED"));

        // Stocker dans IMPACT_GRAPH pour tester execute
        {
            let mut global = IMPACT_GRAPH.write().unwrap();
            *global = graph;
        }

        // Test de requête sur un événement
        let res_event = execute(json!({ "target": "EVENT_CREATED" })).unwrap();
        let text_event = &res_event.content[0].text;
        eprintln!("Event Query Output:\n{}", text_event);
        assert!(text_event.contains("ÉVÉNEMENT DISTRIBUÉ : EVENT_CREATED"));
        assert!(text_event.contains("IEventCreatedPayload"));
        assert!(text_event.contains("Triade de Saga"));

        // Test de requête sur un job
        let res_job = execute(json!({ "target": "PUBLISH_EVENT" })).unwrap();
        let text_job = &res_job.content[0].text;
        eprintln!("Job Query Output:\n{}", text_job);
        assert!(text_job.contains("JOB D'ARRIÈRE-PLAN (1:1) : PUBLISH_EVENT"));

        // Test de recherche inversée
        let res_rev = execute(json!({ "target": "EventCreatedPostProcessor" })).unwrap();
        let text_rev = &res_rev.content[0].text;
        eprintln!("Reverse Query Output:\n{}", text_rev);
        assert!(text_rev.contains("NAVIGATION INVERSÉE"));
        assert!(text_rev.contains("EVENT_CREATED"));
    }
}

