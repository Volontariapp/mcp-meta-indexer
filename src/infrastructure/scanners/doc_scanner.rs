use std::fs;
use std::path::Path;
use ignore::WalkBuilder;

use crate::domain::doc_entry::{DocDocument, DocIndex, DocSection};

pub fn scan_docs(root_dir: &str) -> DocIndex {
    let mut index = DocIndex::default();
    let root_clean = root_dir.trim_end_matches('/');

    let docs_dir = format!("{}/docs", root_clean);
    if !Path::new(&docs_dir).exists() {
        return index;
    }

    let walker = WalkBuilder::new(&docs_dir)
        .hidden(false)
        .git_ignore(false)
        .build();

    for result in walker.flatten() {
        if result.file_type().is_some_and(|ft| ft.is_file()) {
            let path = result.path();
            let path_str = path.strip_prefix(root_clean).unwrap_or(path).to_string_lossy().into_owned();

            if path_str.ends_with(".md") {
                if let Ok(content) = fs::read_to_string(path) {
                    let file_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                    let mut doc = DocDocument {
                        title: file_name.clone(),
                        file_name: file_name.clone(),
                        file_path: path_str.clone(),
                        sections: Vec::new(),
                    };

                    let mut current_section: Option<DocSection> = None;
                    let mut current_lines = Vec::new();

                    for (line_idx, line) in content.lines().enumerate() {
                        let trimmed = line.trim();
                        if trimmed.starts_with('#') {
                            // Sauvegarder la section précédente si elle existe
                            if let Some(mut sec) = current_section.take() {
                                sec.content = current_lines.join("\n").trim().to_string();
                                if !sec.content.is_empty() {
                                    doc.sections.push(sec.clone());
                                    index.sections.push(sec);
                                }
                                current_lines.clear();
                            }

                            let level = trimmed.chars().take_while(|&c| c == '#').count();
                            let title = trimmed.trim_start_matches('#').trim().to_string();

                            if level == 1 && doc.title == file_name {
                                doc.title = title.clone();
                            }

                            current_section = Some(DocSection {
                                title,
                                level,
                                file_name: file_name.clone(),
                                file_path: path_str.clone(),
                                content: String::new(),
                                line_number: line_idx + 1,
                            });
                        } else if current_section.is_some() {
                            current_lines.push(line);
                        }
                    }

                    // Dernière section
                    if let Some(mut sec) = current_section.take() {
                        sec.content = current_lines.join("\n").trim().to_string();
                        if !sec.content.is_empty() {
                            doc.sections.push(sec.clone());
                            index.sections.push(sec);
                        }
                    }

                    index.documents.push(doc);
                }
            }
        }
    }

    index
}
