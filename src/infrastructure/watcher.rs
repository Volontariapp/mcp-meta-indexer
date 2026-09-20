use std::path::Path;
use std::sync::Arc;
use notify::{Watcher, RecursiveMode, Event, EventKind};

use crate::engine::state::AppState;

pub fn start_workspace_watcher(root_dir: String, state: Arc<AppState>) {
    let root_path = Path::new(&root_dir);
    let (tx, rx) = std::sync::mpsc::channel();

    let mut watcher = match notify::recommended_watcher(tx) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("⚠️ Impossible d'initialiser le watcher notify: {}", e);
            return;
        }
    };

    if let Err(e) = watcher.watch(root_path, RecursiveMode::Recursive) {
        eprintln!("⚠️ Impossible de watcher le répertoire {}: {}", root_dir, e);
        return;
    }

    eprintln!("👀 File watcher unifié actif sur '{}'", root_dir);

    for res in rx {
        match res {
            Ok(Event { kind: EventKind::Modify(_), paths, .. })
            | Ok(Event { kind: EventKind::Create(_), paths, .. })
            | Ok(Event { kind: EventKind::Remove(_), paths, .. }) => {
                let mut should_rescan_async = false;
                let mut should_rescan_grpc = false;
                let mut should_rescan_docs = false;

                for p in &paths {
                    let s = p.to_string_lossy();
                    if s.contains("node_modules") || s.contains(".git") || s.contains("/target/") {
                        continue;
                    }

                    if s.ends_with(".proto") {
                        should_rescan_grpc = true;
                    } else if s.ends_with(".md") && s.contains("/docs/") {
                        should_rescan_docs = true;
                    } else if s.ends_with(".ts") || s.ends_with(".tsx") {
                        if s.contains("messaging") || s.contains("post-processor") || s.contains("worker") || s.contains("outbox") {
                            should_rescan_async = true;
                        }
                        if s.contains("controller") || s.contains("service") {
                            should_rescan_grpc = true;
                        }
                    }
                }

                if should_rescan_async {
                    state.reload_async(&root_dir);
                }
                if should_rescan_grpc {
                    state.reload_grpc(&root_dir);
                }
                if should_rescan_docs {
                    state.reload_docs(&root_dir);
                }
            }
            Err(e) => eprintln!("Watcher error: {:?}", e),
            _ => {}
        }
    }
}
