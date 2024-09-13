use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StatusMessage {
    pub status: Value,
    pub sid: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutionStartMessage {
    pub prompt_id: String,
    pub timestamp: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutionCachedMessage {
    pub nodes: Vec<String>,
    pub prompt_id: String,
    pub timestamp: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutingMessage {
    pub node: String,
    pub display_node: String,
    pub prompt_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProgressMessage {
    pub value: usize,
    pub max: usize,
    pub prompt_id: String,
    pub node: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutionSuccessMessage {
    pub prompt_id: String,
    pub timestamp: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutionErrorMessage {
    pub prompt_id: String,
    pub timestamp: usize,
    pub node_id: String,
    pub node_type: String,
    pub exception_type: Option<String>,
    pub exception_message: Option<String>,
    // there are other params that can be ignored for now
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type", content = "data")]
#[non_exhaustive]
pub enum WorkflowMessage {
    Status(StatusMessage),
    ExecutionStart(ExecutionStartMessage),
    ExecutionCached(ExecutionCachedMessage),
    Executing(ExecutingMessage),
    Progress(ProgressMessage),
    ExecutionSuccess(ExecutionSuccessMessage),
    ExecutionError(ExecutionErrorMessage),
}
