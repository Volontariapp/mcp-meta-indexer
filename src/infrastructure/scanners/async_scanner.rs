use std::fs;
use std::path::Path;
use ignore::WalkBuilder;
use regex::Regex;

use crate::domain::async_flow::{AsyncFlowGraph, ConsumerEndpoint, EventNode, FlowEndpoint, JobNode};

pub fn extract_service_name(path: &str) -> String {
    for part in path.split('/') {
        if part.starts_with("ms-")
            || part.starts_with("post-processor-")
            || part.starts_with("worker-")
            || part.starts_with("outbox-")
            || part.starts_with("domain-")
            || part == "ws-service"
            || part == "api-gateway"
        {
            return part.to_string();
        }
    }
    "unknown".to_string()
}

pub fn scan_async_flow(root_dir: &str) -> AsyncFlowGraph {
    let mut graph = AsyncFlowGraph::default();
    let root_clean = root_dir.trim_end_matches('/');

    let re_enum_entry = Regex::new(r#"(?m)^\s*([A-Z0-9_]+)\s*[:=]\s*['"]([^'"]+)['"]"#).unwrap();
    let re_registry_entry = Regex::new(r"(?m)\[([A-Za-z0-9_.]+)\]\s*:\s*([A-Za-z0-9_]+)").unwrap();
    let re_create_event = Regex::new(r"EventQueueEntity\.createEvent<([^>]+)>").unwrap();
    let re_create_event_simple = Regex::new(r"EventQueueEntity\.createEvent").unwrap();
    let re_create_job_simple = Regex::new(r"JobsOutboxEntity\.createJob|withFallback").unwrap();
    let re_job_target = Regex::new(r"JobsOutboxEntity\.createJob<([^>]+)>").unwrap();
    let re_fallback_target = Regex::new(r"withFallback\s*(?:<[^>]+>)?\s*\(\s*([A-Za-z0-9_.]+)").unwrap();
    let re_post_processor = Regex::new(r"(?m)export\s+class\s+([A-Za-z0-9_]+)\s+extends\s+(BatchPostProcessor|SinglePostProcessor)<([^>]+)>").unwrap();
    let re_job_handler = Regex::new(r"(?m)export\s+class\s+([A-Za-z0-9_]+)\s+implements\s+IJobHandler<([^>]+)>").unwrap();
    let re_job_type_field = Regex::new(r"(?m)readonly\s+jobType\s*=\s*([A-Za-z0-9_.]+);").unwrap();

    // 1. Scan SSOT dans npm-packages/packages/messaging
    let messaging_path = format!("{}/npm-packages/packages/messaging", root_clean);
    if Path::new(&messaging_path).exists() {
        let walker = WalkBuilder::new(&messaging_path)
            .hidden(false)
            .git_ignore(false)
            .build();

        for result in walker.flatten() {
            if result.file_type().is_some_and(|ft| ft.is_file()) {
                let path = result.path();
                let path_str = path.strip_prefix(root_clean).unwrap_or(path).to_string_lossy().into_owned();
                if path_str.ends_with(".ts") && !path_str.ends_with(".d.ts") {
                    if let Ok(content) = fs::read_to_string(path) {
                        if path_str.contains("/events/") || path_str.contains("\\events\\") {
                            for cap in re_enum_entry.captures_iter(&content) {
                                let key = cap[1].to_string();
                                let val = cap[2].to_string();
                                let entry = graph.events.entry(key.clone()).or_insert_with(|| EventNode {
                                    event_key: key.clone(),
                                    ..Default::default()
                                });
                                entry.raw_value = Some(val);
                                entry.definition_file = Some(path_str.clone());
                            }

                            for cap in re_registry_entry.captures_iter(&content) {
                                let full_key = cap[1].to_string();
                                let iface = cap[2].to_string();
                                let short_key = full_key.split('.').next_back().unwrap_or(&full_key);
                                if let Some(node) = graph.events.get_mut(short_key) {
                                    node.payload_interface = Some(iface);
                                }
                            }
                        }

                        if path_str.contains("/jobs/") || path_str.contains("\\jobs\\") {
                            for cap in re_enum_entry.captures_iter(&content) {
                                let key = cap[1].to_string();
                                let val = cap[2].to_string();
                                let entry = graph.jobs.entry(key.clone()).or_insert_with(|| JobNode {
                                    job_key: key.clone(),
                                    ..Default::default()
                                });
                                entry.raw_value = Some(val);
                                entry.definition_file = Some(path_str.clone());
                            }

                            for cap in re_registry_entry.captures_iter(&content) {
                                let full_key = cap[1].to_string();
                                let iface = cap[2].to_string();
                                let short_key = full_key.split('.').next_back().unwrap_or(&full_key);
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

    // 2. Scan de toute la codebase pour émetteurs et consommateurs
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
        if result.file_type().is_some_and(|ft| ft.is_file()) {
            let path = result.path();
            let path_str = path.strip_prefix(root_clean).unwrap_or(path).to_string_lossy().into_owned();
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
                    let short_type = generic_type.split('.').next_back().unwrap_or(generic_type).to_string();
                    let match_start = cap.get(0).unwrap().start();
                    let match_end = cap.get(0).unwrap().end();
                    let line_num = content[..match_start].lines().count() + 1;
                    let slice_after = &content[match_end..content.len().min(match_end + 800)];

                    let endpoint = FlowEndpoint {
                        service: extract_service_name(&path_str),
                        file_path: path_str.clone(),
                        line_number: line_num,
                        pattern: "EventQueueEntity.createEvent".to_string(),
                        snippet: slice_after.chars().take(180).collect(),
                    };

                    let mut matched_key = None;
                    for (k, v) in &graph.events {
                        if matches_event_or_payload(k, v, &short_type) {
                            matched_key = Some(k.clone());
                            break;
                        }
                    }

                    let event_key = matched_key.unwrap_or(short_type);
                    let node = graph.events.entry(event_key).or_insert_with(EventNode::default);
                    node.producers.push(endpoint);
                }
            }

            // B. Détection des Émetteurs de Jobs
            if re_create_job_simple.is_match(&content) {
                for cap in re_job_target.captures_iter(&content) {
                    let generic_type = cap[1].trim();
                    let short_type = generic_type.split('.').next_back().unwrap_or(generic_type).to_string();
                    let match_start = cap.get(0).unwrap().start();
                    let match_end = cap.get(0).unwrap().end();
                    let line_num = content[..match_start].lines().count() + 1;
                    let slice_after = &content[match_end..content.len().min(match_end + 800)];

                    let endpoint = FlowEndpoint {
                        service: extract_service_name(&path_str),
                        file_path: path_str.clone(),
                        line_number: line_num,
                        pattern: "JobsOutboxEntity.createJob".to_string(),
                        snippet: slice_after.chars().take(180).collect(),
                    };

                    let mut matched_key = None;
                    for (k, v) in &graph.jobs {
                        if matches_job_or_payload(k, v, &short_type) {
                            matched_key = Some(k.clone());
                            break;
                        }
                    }

                    let job_key = matched_key.unwrap_or(short_type);
                    let node = graph.jobs.entry(job_key).or_insert_with(JobNode::default);
                    node.producers.push(endpoint);
                }

                for cap in re_fallback_target.captures_iter(&content) {
                    let target_job = cap[1].trim().split('.').next_back().unwrap_or(&cap[1]).to_string();
                    let match_start = cap.get(0).unwrap().start();
                    let line_num = content[..match_start].lines().count() + 1;

                    let endpoint = FlowEndpoint {
                        service: extract_service_name(&path_str),
                        file_path: path_str.clone(),
                        line_number: line_num,
                        pattern: "withFallback".to_string(),
                        snippet: String::new(),
                    };

                    let mut matched_key = None;
                    for (k, v) in &graph.jobs {
                        if matches_job_or_payload(k, v, &target_job) {
                            matched_key = Some(k.clone());
                            break;
                        }
                    }

                    let job_key = matched_key.unwrap_or(target_job);
                    let node = graph.jobs.entry(job_key).or_insert_with(JobNode::default);
                    node.producers.push(endpoint);
                }
            }

            // C. Détection des Consommateurs Post-Processors
            for cap in re_post_processor.captures_iter(&content) {
                let class_name = cap[1].to_string();
                let proc_type = cap[2].to_string();
                let payload_type = cap[3].trim();
                let short_payload = payload_type.split('.').next_back().unwrap_or(payload_type).to_string();
                let match_start = cap.get(0).unwrap().start();
                let line_num = content[..match_start].lines().count() + 1;

                let consumer = ConsumerEndpoint {
                    service: extract_service_name(&path_str),
                    handler_class: class_name.clone(),
                    file_path: path_str.clone(),
                    line_number: line_num,
                    processing_type: proc_type,
                };

                let mut matched_event = None;
                for (k, v) in &graph.events {
                    if matches_event_or_payload(k, v, &short_payload) {
                        matched_event = Some(k.clone());
                        break;
                    }
                }

                let target_event = matched_event.unwrap_or_else(|| {
                    short_payload.trim_start_matches('I').trim_end_matches("Payload").to_string()
                });

                graph.consumer_to_target.entry(class_name).or_default().push(target_event.clone());
                let node = graph.events.entry(target_event).or_insert_with(EventNode::default);
                node.consumers.push(consumer);
            }

            // D. Détection des Workers
            for cap in re_job_handler.captures_iter(&content) {
                let class_name = cap[1].to_string();
                let payload_type = cap[2].trim();
                let short_payload = payload_type.split('.').next_back().unwrap_or(payload_type).to_string();
                let match_start = cap.get(0).unwrap().start();
                let line_num = content[..match_start].lines().count() + 1;

                let mut job_key_from_field = None;
                if let Some(field_cap) = re_job_type_field.captures(&content) {
                    let field_val = field_cap[1].trim();
                    job_key_from_field = Some(field_val.split('.').next_back().unwrap_or(field_val).to_string());
                }

                let handler = ConsumerEndpoint {
                    service: extract_service_name(&path_str),
                    handler_class: class_name.clone(),
                    file_path: path_str.clone(),
                    line_number: line_num,
                    processing_type: "IJobHandler".to_string(),
                };

                let mut matched_job = None;
                if let Some(jk) = &job_key_from_field {
                    for (k, v) in &graph.jobs {
                        if matches_job_or_payload(k, v, jk) {
                            matched_job = Some(k.clone());
                            break;
                        }
                    }
                }
                if matched_job.is_none() {
                    for (k, v) in &graph.jobs {
                        if matches_job_or_payload(k, v, &short_payload) {
                            matched_job = Some(k.clone());
                            break;
                        }
                    }
                }

                let target_job = matched_job.unwrap_or_else(|| {
                    job_key_from_field.unwrap_or_else(|| {
                        short_payload.trim_start_matches('I').trim_end_matches("Payload").to_string()
                    })
                });

                graph.consumer_to_target.entry(class_name).or_default().push(target_job.clone());
                let node = graph.jobs.entry(target_job).or_insert_with(JobNode::default);
                node.handlers.push(handler);
            }
        }
    }

    // 3. Liaison automatique des triades de Sagas
    let all_keys: Vec<String> = graph.events.keys().cloned().collect();
    for key in &all_keys {
        if key.ends_with("_CREATED") || key.ends_with("_START") || key.ends_with("_INITIATED") {
            let base_key = if let Some(stripped) = key.strip_suffix("_CREATED") {
                stripped
            } else if let Some(stripped) = key.strip_suffix("_START") {
                stripped
            } else {
                key.strip_suffix("_INITIATED").unwrap_or(key)
            };

            let success_candidate = format!("{}_SUCCESSFULL", base_key);
            let success_alt = format!("{}_SUCCESS", base_key);
            let failure_candidate = format!("{}_FAILED", base_key);
            let failure_alt = format!("{}_FAILURE", base_key);

            let mut succ = None;
            if all_keys.contains(&success_candidate) {
                succ = Some(success_candidate);
            } else if all_keys.contains(&success_alt) {
                succ = Some(success_alt);
            }

            let mut fail = None;
            if all_keys.contains(&failure_candidate) {
                fail = Some(failure_candidate);
            } else if all_keys.contains(&failure_alt) {
                fail = Some(failure_alt);
            }

            if let Some(event_node) = graph.events.get_mut(key) {
                event_node.saga_success = succ;
                event_node.saga_failure = fail;
            }
        }
    }

    graph
}

fn matches_event_or_payload(event_key: &str, node: &EventNode, query: &str) -> bool {
    if event_key.eq_ignore_ascii_case(query) {
        return true;
    }
    if let Some(iface) = &node.payload_interface {
        if iface.eq_ignore_ascii_case(query) {
            return true;
        }
    }
    if let Some(raw) = &node.raw_value {
        if raw.eq_ignore_ascii_case(query) {
            return true;
        }
    }
    let norm_query = query.replace('_', "").to_uppercase();
    let norm_key = event_key.replace('_', "").to_uppercase();
    if norm_query == norm_key {
        return true;
    }
    if norm_query.starts_with('I') && norm_query.ends_with("PAYLOAD") && norm_query.len() > 8 {
        let inner = &norm_query[1..norm_query.len() - 7];
        if inner == norm_key {
            return true;
        }
    }
    false
}

fn matches_job_or_payload(job_key: &str, node: &JobNode, query: &str) -> bool {
    if job_key.eq_ignore_ascii_case(query) {
        return true;
    }
    if let Some(iface) = &node.payload_interface {
        if iface.eq_ignore_ascii_case(query) {
            return true;
        }
    }
    if let Some(raw) = &node.raw_value {
        if raw.eq_ignore_ascii_case(query) {
            return true;
        }
    }
    let norm_query = query.replace('_', "").to_uppercase();
    let norm_key = job_key.replace('_', "").to_uppercase();
    if norm_query == norm_key {
        return true;
    }
    if norm_query.starts_with('I') && norm_query.ends_with("PAYLOAD") && norm_query.len() > 8 {
        let inner = &norm_query[1..norm_query.len() - 7];
        if inner == norm_key {
            return true;
        }
    }
    false
}
