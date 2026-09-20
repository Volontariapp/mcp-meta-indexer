use crate::domain::async_flow::AsyncFlowGraph;

pub fn query_impact(graph: &AsyncFlowGraph, target: &str) -> String {
    let mut out = String::new();
    let target_upper = target.to_uppercase();
    let target_lower = target.to_lowercase();
    let mut found = false;

    // 1. Recherche par Événement
    for (key, event) in &graph.events {
        let matches_key = key.to_uppercase().contains(&target_upper);
        let matches_raw = event.raw_value.as_ref().is_some_and(|r| r.to_lowercase().contains(&target_lower));

        if matches_key || matches_raw {
            found = true;
            out.push_str("================================================================================\n");
            out.push_str(&format!("⚡ ÉVÉNEMENT DISTRIBUÉ : {}\n", key));
            if let Some(raw) = &event.raw_value {
                out.push_str(&format!("   Valeur Bus : '{}'\n", raw));
            }
            out.push_str("================================================================================\n");

            out.push_str("📜 Contrat & Payload :\n");
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
                out.push_str("\n🔄 Triade de Saga (Chorégraphie & Compensations) :\n");
                if let Some(succ) = &event.saga_success {
                    out.push_str(&format!("   - Succès (Commit -> SagaStatus.DONE)   : {}\n", succ));
                }
                if let Some(fail) = &event.saga_failure {
                    out.push_str(&format!("   - Échec  (Rollback -> SagaStatus.CANCEL): {}\n", fail));
                }
            }

            // WebSocket Scatter-Gather
            if let Some(ws) = &event.websocket_event {
                out.push_str(&format!("\n📡 Broadcast WebSocket Mobile (Scatter-Gather) : {}\n", ws));
            }

            out.push('\n');
        }
    }

    // 2. Recherche par Job BullMQ
    for (key, job) in &graph.jobs {
        let matches_key = key.to_uppercase().contains(&target_upper);
        let matches_raw = job.raw_value.as_ref().is_some_and(|r| r.to_lowercase().contains(&target_lower));

        if matches_key || matches_raw {
            found = true;
            out.push_str("================================================================================\n");
            out.push_str(&format!("💼 TÂCHE ASYNCHRONE / JOB (BullMQ) : {}\n", key));
            if let Some(raw) = &job.raw_value {
                out.push_str(&format!("   Valeur Queue : '{}'\n", raw));
            }
            out.push_str("================================================================================\n");

            out.push_str("📜 Contrat & Payload :\n");
            if let Some(iface) = &job.payload_interface {
                out.push_str(&format!("   - Interface Payload : {}\n", iface));
            }
            if let Some(file) = &job.definition_file {
                out.push_str(&format!("   - Définition : {}\n", file));
            }

            out.push_str(&format!("\n🚀 Émetteur(s) (Amont / Producers) : {}\n", job.producers.len()));
            if job.producers.is_empty() {
                out.push_str("   (Aucun émetteur direct via JobsOutboxEntity)\n");
            } else {
                for (i, p) in job.producers.iter().enumerate() {
                    out.push_str(&format!("   {}. [{}] {}\n", i + 1, p.service, p.pattern));
                    out.push_str(&format!("      Fichier : {}:{}\n", p.file_path, p.line_number));
                }
            }

            out.push_str(&format!("\n⚙️ Worker Handler(s) (Aval / Executeurs) : {}\n", job.handlers.len()));
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

    // 3. Recherche inversée depuis un Handler / Post-Processor
    if !found {
        if let Some(targets) = graph.consumer_to_target.get(target) {
            found = true;
            out.push_str(&format!("🔍 Classe consommatrice '{}' identifiée. Dépendance vers :\n", target));
            for t in targets {
                out.push_str(&format!("   - Target : {}\n", t));
                let sub_query = query_impact(graph, t);
                out.push_str(&sub_query);
            }
        }
    }

    if !found {
        format!(
            "Aucun événement, job ou post-processor ne correspond à '{}' dans le graphe causal.",
            target
        )
    } else {
        out
    }
}
