use std::collections::HashMap;
use std::fs;
use std::path::Path;
use ignore::WalkBuilder;
use regex::Regex;

use crate::domain::grpc_flow::{
    GrpcClientEndpoint, GrpcFlowGraph, GrpcMethodNode, GrpcServerEndpoint, GrpcServiceNode,
};
use super::async_scanner::extract_service_name;

fn normalize_symbol(s: &str) -> String {
    s.trim_matches('\'')
        .trim_matches('"')
        .replace('_', "")
        .to_lowercase()
}

pub fn scan_grpc_flow(root_dir: &str) -> GrpcFlowGraph {
    let mut graph = GrpcFlowGraph::default();
    let root_clean = root_dir.trim_end_matches('/');

    let re_proto_package = Regex::new(r"(?m)^\s*package\s+([a-zA-Z0-9_.]+)\s*;").unwrap();
    let re_proto_service_start = Regex::new(r"^\s*service\s+([a-zA-Z0-9_]+)").unwrap();
    let re_proto_rpc = Regex::new(r"^\s*rpc\s+([a-zA-Z0-9_]+)\s*\(\s*([a-zA-Z0-9_.]+)\s*\)\s*returns\s*\(\s*([a-zA-Z0-9_.]+)\s*\)").unwrap();

    let re_grpc_method_literal = Regex::new(r#"@GrpcMethod\s*\(\s*([a-zA-Z0-9_'"\.]+)\s*,\s*([a-zA-Z0-9_'"\.]+)\s*\)"#).unwrap();
    let re_class_name = Regex::new(r"(?m)export\s+class\s+([a-zA-Z0-9_]+)").unwrap();
    let re_grpc_inject = Regex::new(r#"@Inject\s*\(\s*([a-zA-Z0-9_]+)\s*\)\s*(?:private|protected|public|readonly)?\s*(?:readonly)?\s*([a-zA-Z0-9_]+)\s*:\s*ClientGrpc"#).unwrap();

    // Regex pour les contrats Gateway (nativapp -> api-gateway)
    let re_gateway_interface = Regex::new(r"(?m)export\s+interface\s+([a-zA-Z0-9_]+(?:Request|Response))\s+extends\s+(?:Omit<)?([a-zA-Z0-9_]+(?:Command|Query|Response))?").unwrap();
    let re_gateway_standalone_interface = Regex::new(r"(?m)export\s+interface\s+([a-zA-Z0-9_]+(?:Request|Response))").unwrap();

    // 1. Scan SSOT dans proto-registry/proto
    let proto_dir = format!("{}/proto-registry/proto", root_clean);
    if Path::new(&proto_dir).exists() {
        let walker = WalkBuilder::new(&proto_dir)
            .hidden(false)
            .git_ignore(false)
            .build();

        for result in walker.flatten() {
            if result.file_type().is_some_and(|ft| ft.is_file()) {
                let path = result.path();
                let path_str = path.strip_prefix(root_clean).unwrap_or(path).to_string_lossy().into_owned();
                if path_str.ends_with(".proto") {
                    if let Ok(content) = fs::read_to_string(path) {
                        let package_name = re_proto_package
                            .captures(&content)
                            .map(|c| c[1].to_string())
                            .unwrap_or_else(|| "default".to_string());

                        let mut current_service: Option<String> = None;

                        for line in content.lines() {
                            let trimmed = line.trim();

                            if let Some(svc_cap) = re_proto_service_start.captures(trimmed) {
                                let service_name = svc_cap[1].to_string();
                                graph.services.entry(service_name.clone()).or_insert_with(|| {
                                    GrpcServiceNode {
                                        service_name: service_name.clone(),
                                        package_name: package_name.clone(),
                                        proto_file: path_str.clone(),
                                        methods: HashMap::new(),
                                    }
                                });
                                current_service = Some(service_name);
                                continue;
                            }

                            if trimmed == "}" {
                                current_service = None;
                                continue;
                            }

                            if let Some(ref svc_name) = current_service {
                                if let Some(rpc_cap) = re_proto_rpc.captures(trimmed) {
                                    let method_name = rpc_cap[1].to_string();
                                    let req_type = rpc_cap[2].to_string();
                                    let resp_type = rpc_cap[3].to_string();

                                    if let Some(svc_node) = graph.services.get_mut(svc_name) {
                                        svc_node.methods.insert(
                                            method_name.clone(),
                                            GrpcMethodNode {
                                                method_name: method_name.clone(),
                                                service_name: svc_name.clone(),
                                                proto_file: path_str.clone(),
                                                request_type: req_type,
                                                response_type: resp_type,
                                                ts_contract_file: None,
                                                gateway_request: None,
                                                gateway_request_file: None,
                                                gateway_response: None,
                                                gateway_response_file: None,
                                                nest_client_interface: Some(format!("{}Client", svc_name)),
                                                clients: Vec::new(),
                                                servers: Vec::new(),
                                            },
                                        );

                                        graph
                                            .method_to_service
                                            .entry(method_name.to_lowercase())
                                            .or_default()
                                            .push(svc_name.clone());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. Scan des contrats Gateway (nativapp -> api-gateway) dans contracts/src/gateway
    let gateway_contracts_dir = format!("{}/npm-packages/packages/contracts/src/gateway", root_clean);
    if Path::new(&gateway_contracts_dir).exists() {
        let walker = WalkBuilder::new(&gateway_contracts_dir)
            .hidden(false)
            .git_ignore(false)
            .build();

        for result in walker.flatten() {
            if result.file_type().is_some_and(|ft| ft.is_file()) {
                let path = result.path();
                let path_str = path.strip_prefix(root_clean).unwrap_or(path).to_string_lossy().into_owned();
                if path_str.ends_with(".ts") && !path_str.ends_with(".d.ts") {
                    if let Ok(content) = fs::read_to_string(path) {
                        for cap in re_gateway_interface.captures_iter(&content) {
                            let gateway_type = cap[1].to_string();
                            let base_command = cap.get(2).map(|m| m.as_str().to_string());

                            let norm_base = base_command.as_ref().map(|b| normalize_symbol(b));

                            for svc in graph.services.values_mut() {
                                for (m_name, m) in svc.methods.iter_mut() {
                                    let matches_cmd = norm_base.as_ref().is_some_and(|nb| {
                                        let norm_req = normalize_symbol(&m.request_type);
                                        let norm_resp = normalize_symbol(&m.response_type);
                                        nb.contains(&norm_req) || norm_req.contains(nb) || nb.contains(&norm_resp)
                                    });

                                    let matches_method = normalize_symbol(&gateway_type).contains(&normalize_symbol(m_name));

                                    if matches_cmd || matches_method {
                                        if gateway_type.ends_with("Request") {
                                            m.gateway_request = Some(gateway_type.clone());
                                            m.gateway_request_file = Some(path_str.clone());
                                        } else if gateway_type.ends_with("Response") {
                                            m.gateway_response = Some(gateway_type.clone());
                                            m.gateway_response_file = Some(path_str.clone());
                                        }
                                        graph.gateway_contract_to_method.insert(
                                            gateway_type.clone(),
                                            (svc.service_name.clone(), m_name.clone()),
                                        );
                                    }
                                }
                            }
                        }

                        // Fallback standalone interface
                        for cap in re_gateway_standalone_interface.captures_iter(&content) {
                            let iface = cap[1].to_string();
                            let norm_iface = normalize_symbol(&iface);

                            for svc in graph.services.values_mut() {
                                for (m_name, m) in svc.methods.iter_mut() {
                                    if norm_iface.contains(&normalize_symbol(m_name)) {
                                        if iface.ends_with("Request") && m.gateway_request.is_none() {
                                            m.gateway_request = Some(iface.clone());
                                            m.gateway_request_file = Some(path_str.clone());
                                        } else if iface.ends_with("Response") && m.gateway_response.is_none() {
                                            m.gateway_response = Some(iface.clone());
                                            m.gateway_response_file = Some(path_str.clone());
                                        }
                                        graph.gateway_contract_to_method.insert(
                                            iface.clone(),
                                            (svc.service_name.clone(), m_name.clone()),
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 3. Scan codebase pour @GrpcMethod (Serveurs) et ClientGrpc (Clients)
    let mut overrides = ignore::overrides::OverrideBuilder::new(root_clean);
    let _ = overrides.add("**/*");
    let _ = overrides.add("!**/node_modules/**");
    let _ = overrides.add("!**/dist/**");
    let _ = overrides.add("!**/target/**");
    let _ = overrides.add("!**/.git/**");
    let override_set = overrides.build().unwrap_or_else(|_| ignore::overrides::Override::empty());

    let walker_code = WalkBuilder::new(root_clean)
        .hidden(false)
        .git_ignore(false)
        .overrides(override_set)
        .build();

    for result in walker_code.flatten() {
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

            let current_class = re_class_name
                .captures(&content)
                .map(|c| c[1].to_string())
                .unwrap_or_else(|| "UnknownClass".to_string());
            let current_service = extract_service_name(&path_str);

            // A. Détection des serveurs @GrpcMethod
            if content.contains("@GrpcMethod") {
                for cap in re_grpc_method_literal.captures_iter(&content) {
                    let raw_svc = &cap[1];
                    let raw_method = &cap[2];

                    let method_key = raw_method.split('.').next_back().unwrap_or(raw_method);
                    let norm_raw_method = normalize_symbol(method_key);
                    let norm_raw_svc = normalize_symbol(raw_svc);

                    let match_start = cap.get(0).unwrap().start();
                    let line_num = content[..match_start].lines().count() + 1;

                    let server_endpoint = GrpcServerEndpoint {
                        service: current_service.clone(),
                        controller_class: current_class.clone(),
                        file_path: path_str.clone(),
                        line_number: line_num,
                    };

                    for svc in graph.services.values_mut() {
                        let norm_svc_name = normalize_symbol(&svc.service_name);
                        let is_svc_match = norm_raw_svc.contains(&norm_svc_name) || norm_svc_name.contains(&norm_raw_svc);

                        for (m_name, m) in svc.methods.iter_mut() {
                            let norm_m_name = normalize_symbol(m_name);
                            if norm_m_name == norm_raw_method && (is_svc_match || norm_raw_svc.contains("service")) {
                                m.servers.push(server_endpoint.clone());
                            }
                        }
                    }
                }
            }

            // B. Détection des clients (@Inject(*_PACKAGE) client: ClientGrpc)
            if content.contains("ClientGrpc") {
                for cap in re_grpc_inject.captures_iter(&content) {
                    let pkg_name = cap[1].to_string();
                    let match_start = cap.get(0).unwrap().start();
                    let line_num = content[..match_start].lines().count() + 1;

                    let client_endpoint = GrpcClientEndpoint {
                        service: current_service.clone(),
                        class_name: current_class.clone(),
                        file_path: path_str.clone(),
                        line_number: line_num,
                    };

                    let norm_pkg = normalize_symbol(&pkg_name).replace("package", "");
                    for svc in graph.services.values_mut() {
                        let norm_svc = normalize_symbol(&svc.service_name);
                        let norm_proto_pkg = normalize_symbol(&svc.package_name);
                        if norm_svc.contains(&norm_pkg) || norm_proto_pkg.contains(&norm_pkg) {
                            for m in svc.methods.values_mut() {
                                m.clients.push(client_endpoint.clone());
                            }
                        }
                    }
                }
            }
        }
    }

    graph
}
