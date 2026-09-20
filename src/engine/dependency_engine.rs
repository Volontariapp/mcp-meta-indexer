use std::fs;
use ignore::WalkBuilder;
use regex::Regex;

use crate::engine::state::AppState;

pub fn extract_imports(content: &str) -> Vec<String> {
    let mut imports = Vec::new();
    let re = Regex::new(
        r#"(?s)import\s+(?:type\s+)?(?:\{([^}]+)\}|([a-zA-Z0-9_*]+))\s+from\s+['"]([^'"]+)['"]"#,
    )
    .unwrap();

    for cap in re.captures_iter(content) {
        if let Some(pkg) = cap.get(3) {
            let pkg_str = pkg.as_str().to_string();
            imports.push(pkg_str.clone());
            // Si c'est un sous-chemin comme @volontariapp/messaging/jobs, indexer aussi le package racine
            if let Some(idx) = pkg_str.rfind('/') {
                if pkg_str.starts_with('@') && pkg_str[idx..].contains('/') {
                    // ex: @scope/pkg/sub -> @scope/pkg
                    let parts: Vec<&str> = pkg_str.split('/').collect();
                    if parts.len() > 2 {
                        imports.push(format!("{}/{}", parts[0], parts[1]));
                    }
                }
            }
        }

        if let Some(symbols_block) = cap.get(1) {
            for symbol in symbols_block.as_str().split(',') {
                let s = symbol.trim().trim_start_matches("type ").trim();
                if !s.is_empty() {
                    let parts: Vec<&str> = s.split(" as ").collect();
                    imports.push(parts[0].trim().to_string());
                }
            }
        }

        if let Some(default_sym) = cap.get(2) {
            imports.push(default_sym.as_str().trim().to_string());
        }
    }

    imports
}

pub fn index_dependencies(state: &AppState, base_path: &str) {
    let mut overrides = ignore::overrides::OverrideBuilder::new(base_path);
    let _ = overrides.add("**/*");
    let _ = overrides.add("!**/node_modules/*/**");
    let _ = overrides.add("**/node_modules/@volontariapp/**");
    let _ = overrides.add("!**/.git/**");
    let override_set = overrides.build().unwrap_or_else(|_| ignore::overrides::Override::empty());

    let walker = WalkBuilder::new(base_path)
        .hidden(false)
        .git_ignore(false)
        .overrides(override_set)
        .build();

    let mut lock = state.dependencies.write().unwrap();
    for entry in walker.flatten() {
        if entry.file_type().is_some_and(|ft| ft.is_file()) {
            let path = entry.path();
            let path_str = path.strip_prefix(base_path).unwrap_or(path).to_string_lossy().into_owned();
            if path_str.ends_with(".ts") || path_str.ends_with(".tsx") {
                if let Ok(content) = fs::read_to_string(path) {
                    for import in extract_imports(&content) {
                        lock.entry(import).or_default().insert(path_str.clone());
                    }
                }
            }
        }
    }
}

pub fn query_dependents(state: &AppState, target: &str) -> Option<Vec<String>> {
    let lock = state.dependencies.read().unwrap();
    lock.get(target).map(|set| {
        let mut list: Vec<String> = set.iter().cloned().collect();
        list.sort();
        list
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_imports_complex() {
        let ts_code = r#"
import { Logger } from '@volontariapp/logger';
import type { UserAuthRequest, SignUpDTO } from '@volontariapp/contracts';
import {
    EventsQueue,
    type ISyncExternalCalendarPayload,
} from '@volontariapp/messaging/jobs';
import * as path from 'path';
"#;
        let imports = extract_imports(ts_code);
        assert!(imports.contains(&"@volontariapp/logger".to_string()));
        assert!(imports.contains(&"Logger".to_string()));
        assert!(imports.contains(&"@volontariapp/contracts".to_string()));
        assert!(imports.contains(&"UserAuthRequest".to_string()));
        assert!(imports.contains(&"SignUpDTO".to_string()));
        assert!(imports.contains(&"@volontariapp/messaging/jobs".to_string()));
        assert!(imports.contains(&"@volontariapp/messaging".to_string()));
        assert!(imports.contains(&"EventsQueue".to_string()));
        assert!(imports.contains(&"ISyncExternalCalendarPayload".to_string()));
    }
}

