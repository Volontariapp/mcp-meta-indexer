use crate::domain::doc_entry::DocIndex;

pub fn query_docs(index: &DocIndex, query: &str, max_sections: usize) -> String {
    let mut out = String::new();
    let q_lower = query.to_lowercase();
    let words: Vec<&str> = q_lower.split_whitespace().collect();

    // Scorer chaque section
    let mut scored_sections: Vec<(&crate::domain::doc_entry::DocSection, usize)> = index
        .sections
        .iter()
        .filter_map(|sec| {
            let mut score = 0;
            let title_lower = sec.title.to_lowercase();
            let content_lower = sec.content.to_lowercase();

            for w in &words {
                if title_lower.contains(w) {
                    score += 10;
                }
                if content_lower.contains(w) {
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

    scored_sections.sort_by(|a, b| b.1.cmp(&a.1));
    scored_sections.truncate(max_sections);

    if scored_sections.is_empty() {
        return format!(
            "Aucune section trouvée pour '{}' dans la documentation C4 (meta/docs/).",
            query
        );
    }

    out.push_str(&format!("📚 Documentation C4 : {} résultat(s) pour '{}'\n\n", scored_sections.len(), query));

    for (sec, _) in scored_sections {
        out.push_str("================================================================================\n");
        out.push_str(&format!("📑 Section : {} (Fichier: {}:{})\n", sec.title, sec.file_path, sec.line_number));
        out.push_str("================================================================================\n");
        
        let content_to_show = if sec.content.len() > 1500 {
            format!("{}\n... (suite tronquée pour économie de tokens)", &sec.content[..1500])
        } else {
            sec.content.clone()
        };
        out.push_str(&content_to_show);
        out.push_str("\n\n");
    }

    out
}
