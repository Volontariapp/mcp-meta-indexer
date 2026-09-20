use std::collections::HashMap;

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct FlowEndpoint {
    pub service: String,
    pub file_path: String,
    pub line_number: usize,
    pub pattern: String,
    pub snippet: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct ConsumerEndpoint {
    pub service: String,
    pub handler_class: String,
    pub file_path: String,
    pub line_number: usize,
    pub processing_type: String, // "BatchPostProcessor" | "SinglePostProcessor" | "IJobHandler"
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct EventNode {
    pub event_key: String,
    pub raw_value: Option<String>,
    pub payload_interface: Option<String>,
    pub definition_file: Option<String>,
    pub stream: Option<String>,
    pub producers: Vec<FlowEndpoint>,
    pub consumers: Vec<ConsumerEndpoint>,
    pub saga_success: Option<String>,
    pub saga_failure: Option<String>,
    pub websocket_event: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct JobNode {
    pub job_key: String,
    pub raw_value: Option<String>,
    pub payload_interface: Option<String>,
    pub definition_file: Option<String>,
    pub queue: Option<String>,
    pub producers: Vec<FlowEndpoint>,
    pub handlers: Vec<ConsumerEndpoint>,
}

#[derive(Debug, Clone, Default)]
pub struct AsyncFlowGraph {
    pub events: HashMap<String, EventNode>,
    pub jobs: HashMap<String, JobNode>,
    pub consumer_to_target: HashMap<String, Vec<String>>,
}
