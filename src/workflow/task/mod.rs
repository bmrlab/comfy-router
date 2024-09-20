pub mod impls;
pub mod executor;

use super::payload::WorkflowPayload;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowRunningResult {
    pub progress: f32,
    pub previews: Vec<Vec<u8>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase", tag = "status", content = "data")]
pub enum WorkflowResult {
    Pending(usize),
    Running(WorkflowRunningResult),
    Done(Vec<Vec<u8>>),
    Error(String),
}

#[derive(Clone, Debug)]
pub struct WorkflowTask {
    id: String,
    payload: WorkflowPayload,
    result: Arc<RwLock<WorkflowResult>>,
}
