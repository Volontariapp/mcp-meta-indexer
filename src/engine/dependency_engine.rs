use std::fs;
use ignore::WalkBuilder;
use regex::Regex;

use crate::engine::state::AppState;

pub fn extract_imports(content: &str) -> Vec<String> {
    let mut imports = Vec::new();
    let re = Regex::new(r#"(?m)^import\s+(?:\{([^}]+)\}|([a-zA-Z0-9_*]+))\s+from\s+['"]([^'"]+)['"]"#).unwrap();

    for cap in re.captures_iter(content) {
        if let Some(pkg) = cap.get(3) {
            imports.push(pkg.as_str().to_string());
        }

        if let Some(symbols_block) = cap.get(1) {
            for symbol in symbols_block.as_str().split(',') {
                let s = symbol.trim();
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
