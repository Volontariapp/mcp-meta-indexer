use crate::domain::grpc_flow::GrpcFlowGraph;

pub fn query_grpc(graph: &GrpcFlowGraph, target: &str) -> String {
    let mut out = String::new();
    let target_lower = target.to_lowercase();
    let mut matched_methods = Vec::new();

    // A. Recherche par contrat Gateway direct (ex: SignUpRequest, CreateEventRequest)
    for (gw_contract, (svc_name, m_name)) in &graph.gateway_contract_to_method {
        if gw_contract.to_lowercase().contains(&target_lower) {
            if let Some(svc) = graph.services.get(svc_name) {
                if let Some(m) = svc.methods.get(m_name) {
                    if !matched_methods.iter().any(|existing: &&crate::domain::grpc_flow::GrpcMethodNode| existing.method_name == m.method_name && existing.service_name == m.service_name) {
                        matched_methods.push(m);
                    }
                }
            }
        }
    }

    // B. Recherche directe par nom de méthode ou service ou request/response gRPC
    for svc in graph.services.values() {
        if svc.service_name.to_lowercase().contains(&target_lower) {
            for m in svc.methods.values() {
                if !matched_methods.iter().any(|existing| existing.method_name == m.method_name && existing.service_name == m.service_name) {
                    matched_methods.push(m);
                }
            }
        } else {
            for (m_name, m) in &svc.methods {
                let matches_name = m_name.to_lowercase().contains(&target_lower);
                let matches_req = m.request_type.to_lowercase().contains(&target_lower);
                let matches_resp = m.response_type.to_lowercase().contains(&target_lower);

                if matches_name || matches_req || matches_resp {
                    if !matched_methods.iter().any(|existing| existing.method_name == m.method_name && existing.service_name == m.service_name) {
                        matched_methods.push(m);
                    }
                }
            }
        }
    }

    if matched_methods.is_empty() {
        return format!(
            "Aucun contrat gRPC, contrat gateway ou méthode ne correspond à '{}' dans proto-registry et contracts.",
            target
        );
    }

    for m in matched_methods {
        out.push_str("================================================================================\n");
        out.push_str(&format!("🌐 RPC SYNCHRONE gRPC : {}.{}\n", m.service_name, m.method_name));
        out.push_str("================================================================================\n");

        // 1. Contrat Mobile/Gateway (nativapp -> api-gateway)
        out.push_str("📱 1. Contrat Mobile / Front (nativapp -> api-gateway) :\n");
        if let Some(gw_req) = &m.gateway_request {
            out.push_str(&format!("   - Request  : {}\n", gw_req));
            if let Some(f) = &m.gateway_request_file {
                out.push_str(&format!("     Fichier  : {}\n", f));
            }
        } else {
            out.push_str("   - Request  : (Utilise directement le format standard gRPC ou DTO interne)\n");
        }
        if let Some(gw_resp) = &m.gateway_response {
            out.push_str(&format!("   - Response : {}\n", gw_resp));
            if let Some(f) = &m.gateway_response_file {
                out.push_str(&format!("     Fichier  : {}\n", f));
            }
        }

        // 2. Contrat gRPC Microservice (api-gateway -> Microservice)
        out.push_str("\n⚙️ 2. Contrat gRPC Inter-Services (api-gateway -> Microservice) :\n");
        out.push_str(&format!("   - Source Proto : {}\n", m.proto_file));
        out.push_str(&format!("   - Command/Query : {}\n", m.request_type));
        out.push_str(&format!("   - Response Proto: {}\n", m.response_type));
        if let Some(ts_f) = &m.ts_contract_file {
            out.push_str(&format!("   - Fichier TS    : {}\n", ts_f));
        }

        // 3. Contrat NestJS (@volontariapp/contracts-nest)
        if let Some(nest_client) = &m.nest_client_interface {
            out.push_str("\n🦅 3. Contrat NestJS (@volontariapp/contracts-nest) :\n");
            out.push_str(&format!("   - Client Interface: {}\n", nest_client));
        }

        // 4. Appelants & Implémentations
        out.push_str(&format!("\n📞 Client(s) Appelant(s) (API Gateway / MS) : {}\n", m.clients.len()));
        if m.clients.is_empty() {
            out.push_str("   (Aucun client injecté direct détecté via ClientGrpc)\n");
        } else {
            for (i, c) in m.clients.iter().enumerate() {
                out.push_str(&format!("   {}. [{}] {}\n", i + 1, c.service, c.class_name));
                out.push_str(&format!("      Fichier : {}:{}\n", c.file_path, c.line_number));
            }
        }

        out.push_str(&format!("\n🎯 Controller(s) Implémentation (Microservice) : {}\n", m.servers.len()));
        if m.servers.is_empty() {
            out.push_str("   (Aucun controller direct détecté avec @GrpcMethod)\n");
        } else {
            for (i, s) in m.servers.iter().enumerate() {
                out.push_str(&format!("   {}. [{}] {}\n", i + 1, s.service, s.controller_class));
                out.push_str(&format!("      Fichier : {}:{}\n", s.file_path, s.line_number));
            }
        }

        out.push('\n');
    }

    out
}
