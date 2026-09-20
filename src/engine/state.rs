use std::collections::{HashMap, HashSet};
use std::sync::RwLock;

use crate::domain::async_flow::AsyncFlowGraph;
use crate::domain::doc_entry::DocIndex;
use crate::domain::grpc_flow::GrpcFlowGraph;
use crate::infrastructure::scanners::{scan_async_flow, scan_docs, scan_grpc_flow};

pub struct AppState {
    pub root_dir: String,
    pub dependencies: RwLock<HashMap<String, HashSet<String>>>,
    pub async_flow: RwLock<AsyncFlowGraph>,
    pub grpc_flow: RwLock<GrpcFlowGraph>,
    pub docs: RwLock<DocIndex>,
}

impl AppState {
    pub fn new(root_dir: String) -> Self {
        let async_flow = scan_async_flow(&root_dir);
        let grpc_flow = scan_grpc_flow(&root_dir);
        let docs = scan_docs(&root_dir);

        Self {
            root_dir,
            dependencies: RwLock::new(HashMap::new()),
            async_flow: RwLock::new(async_flow),
            grpc_flow: RwLock::new(grpc_flow),
            docs: RwLock::new(docs),
        }
    }

    pub fn reload_async(&self, root_dir: &str) {
        let updated = scan_async_flow(root_dir);
        let mut lock = self.async_flow.write().unwrap();
        *lock = updated;
        eprintln!("🔄 Async flow graph rechargé en RAM.");
    }

    pub fn reload_grpc(&self, root_dir: &str) {
        let updated = scan_grpc_flow(root_dir);
        let mut lock = self.grpc_flow.write().unwrap();
        *lock = updated;
        eprintln!("🔄 gRPC flow graph rechargé en RAM.");
    }

    pub fn reload_docs(&self, root_dir: &str) {
        let updated = scan_docs(root_dir);
        let mut lock = self.docs.write().unwrap();
        *lock = updated;
        eprintln!("🔄 Documentation C4 rechargée en RAM.");
    }
}
