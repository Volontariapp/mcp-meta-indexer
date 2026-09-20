use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct GrpcClientEndpoint {
    pub service: String,
    pub class_name: String,
    pub file_path: String,
    pub line_number: usize,
}

#[derive(Debug, Clone, Default)]
pub struct GrpcServerEndpoint {
    pub service: String,
    pub controller_class: String,
    pub file_path: String,
    pub line_number: usize,
}

#[derive(Debug, Clone, Default)]
pub struct GrpcMethodNode {
    pub method_name: String,
    pub service_name: String,
    pub proto_file: String,
    
    // Contrat gRPC (api-gateway -> Microservice)
    pub request_type: String,
    pub response_type: String,
    pub ts_contract_file: Option<String>,
    
    // Contrat Gateway HTTP/REST (nativapp -> api-gateway)
    pub gateway_request: Option<String>,
    pub gateway_request_file: Option<String>,
    pub gateway_response: Option<String>,
    pub gateway_response_file: Option<String>,
    
    // Contrat NestJS (@volontariapp/contracts-nest)
    pub nest_client_interface: Option<String>,

    pub clients: Vec<GrpcClientEndpoint>,
    pub servers: Vec<GrpcServerEndpoint>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct GrpcServiceNode {
    pub service_name: String,
    pub package_name: String,
    pub proto_file: String,
    pub methods: HashMap<String, GrpcMethodNode>,
}

#[derive(Debug, Clone, Default)]
pub struct GrpcFlowGraph {
    pub services: HashMap<String, GrpcServiceNode>,
    /// Index inversé méthode / contrat -> service pour lookup direct O(1)
    pub method_to_service: HashMap<String, Vec<String>>,
    /// Index inversé contrat gateway (ex: SignUpRequest) -> (service, méthode)
    pub gateway_contract_to_method: HashMap<String, (String, String)>,
}
