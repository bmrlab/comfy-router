pub mod executor;
pub mod impls;

use super::payload::WorkflowPayload;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use utoipa::ToSchema;

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct WorkflowRunningResult {
    pub progress: f32,
    pub previews: Vec<Vec<u8>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase", tag = "status", content = "data")]
pub enum WorkflowResult {
    #[schema(value_type = u32, default = u32::default)]
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
