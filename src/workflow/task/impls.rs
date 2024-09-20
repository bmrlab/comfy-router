use super::{WorkflowResult, WorkflowTask};
use crate::{
    state::AppState,
    workflow::{
        payload::{generate_comfy_prompt, WorkflowPayload},
        task::executor::TaskExecutor,
    },
};
use std::sync::Arc;
use tokio::sync::RwLock;
use url::Url;

impl WorkflowTask {
    pub fn new(payload: WorkflowPayload) -> Self {
        let result = Arc::new(RwLock::new(WorkflowResult::Pending(0)));

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            payload,
            result,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn payload(&self) -> &WorkflowPayload {
        &self.payload
    }

    pub async fn result(&self) -> WorkflowResult {
        self.result.read().await.clone()
    }

    #[tracing::instrument(skip_all, fields(task_id = self.id))]
    pub async fn run(&self, node: &Url, app_state: Arc<AppState>) {
        let prompt = generate_comfy_prompt(&self.payload, app_state).await;

        tracing::info!("got prompt");

        let mut executor = TaskExecutor::new(prompt, self.result.clone(), self.id());
        match executor.run(node).await {
            Err(e) => {
                let mut result = self.result.write().await;
                *result = WorkflowResult::Error(e.to_string());
            }
            _ => {}
        }
    }
}
