use crate::domain::doc_entry::{DocIndex, DocSection};
use crate::engine::fuzzy_engine::FuzzyEngine;

const STOP_WORDS: &[&str] = &[
    "le", "la", "les", "de", "des", "du", "un", "une", "et", "en", "pour", "dans", "sur", "au",
    "aux", "ce", "cet", "cette", "ces", "par", "est", "sont", "avec", "the", "a", "an", "and",
    "or", "in", "on", "at", "to", "for", "of", "with", "is", "are", "it", "how", "what", "why",
    "where", "comment", "pourquoi", "qui", "quel", "quelle", "quels", "quelles", "c4", "doc",
    "docs",
];

fn expand_synonyms(word: &str) -> Vec<&'static str> {
    match word {
        "k8s" | "kube" => vec!["kubernetes", "deployment", "argocd", "ingress", "pod"],
        "postgres" | "sql" | "db" | "database" => {
            vec!["postgresql", "acid", "transaction", "outbox", "typeorm"]
        }
        "ws" | "socket" | "realtime" => {
            vec!["websocket", "scatter-gather", "ws-service", "gateway"]
        }
        "proto" | "protobuf" => vec!["grpc", "proto-registry", "contracts", "contracts-nest"],
        "dlq" => vec!["dead", "letter", "queue", "retry", "failure"],
        "jwt" | "token" => vec!["internal_token", "auth", "secret", "zero-trust"],
        "job" | "jobs" => vec!["bullmq", "worker", "jobs_outbox", "job_audit"],
        "event" | "events" => vec!["stream", "post-processor", "event_outbox", "saga"],
        "saga" | "sagas" => vec!["compensation", "rollback", "choreography", "status"],
        "npm" | "pkg" | "package" | "packages" => {
            vec!["monorepo", "changeset", "contracts", "domain"]
        }
        _ => vec![],
    }
}

fn format_content_snippet(content: &str, query: &str, words: &[&str]) -> String {
    if content.len() <= 1500 {
        return content.to_string();
    }

    let content_lower = content.to_lowercase();
    let match_pos = content_lower.find(query).or_else(|| {
        words
            .iter()
            .filter_map(|w| content_lower.find(w))
            .min()
    });

    if let Some(pos) = match_pos {
        let start = pos.saturating_sub(250);
        let aligned_start = content[start..pos]
            .rfind('\n')
            .map(|idx| start + idx + 1)
            .unwrap_or(start);

        let end = (aligned_start + 1300).min(content.len());
        let aligned_end = content[pos..end]
            .rfind('\n')
            .map(|idx| pos + idx)
            .unwrap_or(end);

        let prefix = if aligned_start > 0 {
            "... (contexte amont)\n"
        } else {
            ""
        };
        let suffix = if aligned_end < content.len() {
            "\n... (suite tronquée)"
        } else {
            ""
        };
        format!("{}{}{}", prefix, &content[aligned_start..aligned_end], suffix)
    } else {
        format!(
            "{}\n... (suite tronquée pour économie de tokens)",
            &content[..1500]
        )
    }
}

fn generate_recommendations(query: &str, matched_sections: &[&DocSection]) -> Option<String> {
    let mut recommendations = Vec::new();
    let q_lower = query.to_lowercase();

    let combined_text: String = matched_sections
        .iter()
        .map(|s| format!("{} {}", s.title.to_lowercase(), s.content.to_lowercase()))
        .collect::<Vec<_>>()
        .join(" ");

    let has_job = q_lower.contains("job")
        || q_lower.contains("bullmq")
        || q_lower.contains("worker")
        || combined_text.contains("jobs_outbox")
        || combined_text.contains("job_audit")
        || combined_text.contains("baseworker");

    let has_event = q_lower.contains("event")
        || q_lower.contains("stream")
        || q_lower.contains("scatter")
        || q_lower.contains("saga")
        || combined_text.contains("event_outbox")
        || combined_text.contains("batchpostprocessor")
        || combined_text.contains("scatter-gather");

    let has_proto = q_lower.contains("proto")
        || q_lower.contains("grpc")
        || combined_text.contains("proto-registry")
        || combined_text.contains("@volontariapp/contracts");

    let has_npm = q_lower.contains("npm")
        || q_lower.contains("package")
        || combined_text.contains("changeset")
        || combined_text.contains("@volontariapp/domain-");

    if has_job {
        recommendations.push("💼 **Flux de Job BullMQ :**\n   - Pour implémenter ce job : Skill `.agents/skills/global/implement-async-job-flow/SKILL.md`\n   - Pour cartographier ou déboguer : MCP `analyze_impact({ target: \"...\" })` ou Skill `trace-async-flow`\n   - 🛑 **RÈGLE DU STOP** : Si tu touches à `messaging`, `yarn changeset add` puis STOP IMMÉDIAT !");
    }

    if has_event {
        recommendations.push("⚡ **Flux Événementiel / Saga / Scatter-Gather :**\n   - Pour implémenter cet événement : Skill `.agents/skills/global/implement-async-event-flow/SKILL.md`\n   - Pour cartographier producteurs/consommateurs : MCP `analyze_impact({ target: \"...\" })`\n   - 🛑 **RÈGLE DU STOP** : Si tu touches à `messaging` ou `shared`, STOP IMMÉDIAT après le changeset !");
    }

    if has_proto {
        recommendations.push("🌐 **Contrats Protobuf & gRPC :**\n   - Pour modifier un contrat : Skill `.agents/skills/global/proto-contract-evolution/SKILL.md`\n   - Pour tracer une méthode RPC : MCP `analyze_grpc({ target: \"...\" })`\n   - 🛑 **RÈGLE BLOQUANTE (CASCADE CI)** : `proto-registry` génère une PR dans `npm-packages` au merge. Attends la publication NPM avant de toucher aux microservices !");
    }

    if has_npm && !has_job && !has_event {
        recommendations.push("📦 **Packages Partagés (Monorepo npm-packages) :**\n   - Pour modifier un paquet partagé : Skill `.agents/skills/global/shared-npm-package-change/SKILL.md`\n   - Pour trouver qui l'importe : MCP `find_dependents({ target: \"...\" })`\n   - 🛑 **RÈGLE DU STOP IMMÉDIAT** : Interdiction formelle de modifier les microservices tant que la CI n'a pas publié le paquet !");
    }

    if recommendations.is_empty() {
        None
    } else {
        Some(format!(
            "════════════════════════════════════════════════════════════════════════════════\n💡 GUIDANCE OPÉRATIONNELLE & SKILLS RECOMMANDÉS :\n{}\n════════════════════════════════════════════════════════════════════════════════\n",
            recommendations.join("\n\n")
        ))
    }
}

pub fn query_docs(
    index: &DocIndex,
    query: &str,
    max_sections: usize,
    root_dir: &str,
) -> String {
    let mut out = String::new();
    let q_clean = query.trim().to_lowercase();
    let raw_words: Vec<&str> = q_clean.split_whitespace().collect();

    // 1. Filtrage des stop words
    let mut filtered_words: Vec<&str> = raw_words
        .iter()
        .copied()
        .filter(|w| !STOP_WORDS.contains(w) && w.len() > 1)
        .collect();

    if filtered_words.is_empty() {
        filtered_words = raw_words.clone();
    }

    // 2. Expansion des synonymes
    let mut all_synonyms = Vec::new();
    for w in &filtered_words {
        all_synonyms.extend(expand_synonyms(w));
    }

    // 3. Scorer chaque section
    let mut scored_sections: Vec<(&DocSection, usize)> = index
        .sections
        .iter()
        .filter_map(|sec| {
            let mut score = 0;
            let title_lower = sec.title.to_lowercase();
            let content_lower = sec.content.to_lowercase();
            let file_lower = sec.file_name.to_lowercase();

            // Bonus Phrase Exacte
            if !q_clean.is_empty() {
                if title_lower.contains(&q_clean) {
                    score += 60;
                } else if content_lower.contains(&q_clean) {
                    score += 25;
                }
            }

            // Bonus Nom de Fichier (ex: C3, Async, Monorepo)
            for w in &filtered_words {
                if file_lower.contains(w) {
                    score += 15;
                }
            }

            // Mots filtrés
            for w in &filtered_words {
                if title_lower.contains(w) {
                    score += 12;
                }
                if title_lower.split_whitespace().any(|t| t == *w) {
                    score += 8;
                }
                if content_lower.contains(w) {
                    score += 3;
                }
            }

            // Synonymes
            for syn in &all_synonyms {
                if title_lower.contains(syn) {
                    score += 6;
                }
                if content_lower.contains(syn) {
                    score += 2;
                }
            }

            if score > 0 {
                Some((sec, score))
            } else {
                None
            }
        })
        .collect();

    scored_sections.sort_by_key(|a| std::cmp::Reverse(a.1));
    scored_sections.truncate(max_sections);

    // 4. Si aucun résultat exact -> Fallback Fuzzy Matcher
    if scored_sections.is_empty() {
        let fuzzy_engine = FuzzyEngine::new();
        let mut candidate_titles = Vec::new();
        for sec in &index.sections {
            if !candidate_titles.contains(&sec.title) {
                candidate_titles.push(sec.title.clone());
            }
        }

        let best_matches = fuzzy_engine.find_best_matches(query, &candidate_titles, 5);

        let mut fallback_text = format!(
            "Aucune section exacte trouvée pour '{}' dans la documentation C4.\n",
            query
        );

        if !best_matches.is_empty() {
            fallback_text.push_str("\n💡 Suggestions lexicales les plus proches dans la documentation :\n");
            for (title, score) in best_matches {
                fallback_text.push_str(&format!("  - `{}` (score: {})\n", title, score));
            }
            fallback_text.push_str("\nTu peux relancer `search_docs` avec l'un de ces termes.");
        }

        return fallback_text;
    }

    out.push_str(&format!(
        "📚 Documentation C4 : {} résultat(s) pertinent(s) pour '{}'\n\n",
        scored_sections.len(),
        query
    ));

    let clean_root = root_dir.trim_end_matches('/');

    for (sec, score) in &scored_sections {
        let clean_path = sec.file_path.trim_start_matches('/');
        let link_url = if sec.file_path.starts_with('/') {
            format!("{}#L{}", sec.file_path, sec.line_number)
        } else {
            format!("{}/{}#L{}", clean_root, clean_path, sec.line_number)
        };

        out.push_str("================================================================================\n");
        out.push_str(&format!(
            "📑 [{}:L{}] {} (Score: {})\n   Lien : file://{}\n",
            sec.file_name, sec.line_number, sec.title, score, link_url
        ));
        out.push_str("================================================================================\n");

        let snippet = format_content_snippet(&sec.content, &q_clean, &filtered_words);
        out.push_str(&snippet);
        out.push_str("\n\n");
    }

    // 5. Recommandations de skills contextuels
    let top_sections: Vec<&DocSection> = scored_sections.iter().map(|(s, _)| *s).collect();
    if let Some(reco) = generate_recommendations(query, &top_sections) {
        out.push_str(&reco);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::doc_entry::DocDocument;

    #[test]
    fn test_exact_phrase_and_synonym_boost() {
        let mut index = DocIndex::default();
        let sec1 = DocSection {
            title: "Scatter-Gather WebSocket".to_string(),
            level: 2,
            file_name: "C3-Async-Patterns-And-Flows.md".to_string(),
            file_path: "docs/C3-Async-Patterns-And-Flows.md".to_string(),
            content: "Le pattern Scatter-Gather permet d'agréger 2 feedbacks.".to_string(),
            line_number: 10,
        };
        let sec2 = DocSection {
            title: "Architecture Générale".to_string(),
            level: 1,
            file_name: "C1-System-Context.md".to_string(),
            file_path: "docs/C1-System-Context.md".to_string(),
            content: "Volontariapp utilise du realtime.".to_string(),
            line_number: 1,
        };

        index.sections.push(sec1);
        index.sections.push(sec2);
        index.documents.push(DocDocument::default());

        // Test avec alias 'ws'
        let res = query_docs(&index, "ws scatter gather", 2, "/workspace");
        assert!(res.contains("Scatter-Gather WebSocket"));
        assert!(res.contains("GUIDANCE OPÉRATIONNELLE"));
    }
}
