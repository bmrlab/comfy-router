use super::WorkflowResult;
use crate::workflow::{
    message::WorkflowMessage, payload::ComfyUIPrompt, task::WorkflowRunningResult,
};
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use url::Url;

#[derive(Error, Debug)]
pub enum WorkflowExecutionError {
    #[error("failed to connect to node using websocket")]
    WebSocketConnectionError,
    #[error("ComfyUI error: {0}")]
    ComfyUIError(String),
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
}

pub struct TaskExecutor {
    prompt: ComfyUIPrompt,
    result: Arc<RwLock<WorkflowResult>>,
    task_id: String,
    prompt_id: String,
    current_node_id: Option<String>,
    results: Vec<Vec<u8>>,
}

impl TaskExecutor {
    pub fn new(prompt: ComfyUIPrompt, result: Arc<RwLock<WorkflowResult>>, task_id: &str) -> Self {
        Self {
            prompt,
            result,
            task_id: task_id.to_string(),
            prompt_id: String::new(),
            current_node_id: None,
            results: vec![],
        }
    }

    async fn trigger_workflow(&self, node: &Url) -> Result<String, WorkflowExecutionError> {
        let client = Client::new();
        let response = client
            .post(node.join("/prompt").expect(""))
            .json(&json!({
                "prompt": &self.prompt.prompt,
                "client_id": &self.task_id
            }))
            .send()
            .await
            .map_err(|e| WorkflowExecutionError::ComfyUIError(e.to_string()))?;

        let response_json = response
            .json::<Value>()
            .await
            .map_err(|e| WorkflowExecutionError::ComfyUIError(e.to_string()))?;
        tracing::debug!("response: {:?}", response_json);

        let response_json =
            response_json
                .as_object()
                .ok_or(WorkflowExecutionError::InvalidResponse(
                    response_json.to_string(),
                ))?;
        let prompt_id =
            response_json
                .get("prompt_id")
                .ok_or(WorkflowExecutionError::InvalidResponse(
                    "cannot find prompt_id".to_string(),
                ))?;
        let prompt_id = prompt_id
            .as_str()
            .ok_or(WorkflowExecutionError::InvalidResponse(format!(
                "invalid prompt_id ({})",
                prompt_id
            )))?;

        Ok(prompt_id.to_string())
    }

    /// Update result when new message come in. Return `true` if workflow is done.
    async fn on_message(&mut self, message: WorkflowMessage) -> bool {
        tracing::debug!("workflow on_message: {:?}", &message);

        match message {
            WorkflowMessage::Status(_) => {
                // safely ignore
            }
            WorkflowMessage::ExecutionStart(data) => {
                if data.prompt_id == self.prompt_id {
                    tracing::debug!("execution start: {:?}", data);
                    // update task status
                    let mut result = self.result.write().await;

                    *result = WorkflowResult::Running(WorkflowRunningResult {
                        progress: 0.0,
                        previews: vec![],
                    });
                }
            }
            WorkflowMessage::ExecutionCached(_) => {
                // safely ignore
            }
            WorkflowMessage::Executing(data) => {
                if data.prompt_id == self.prompt_id {
                    self.current_node_id = Some(data.node);
                }
            }
            WorkflowMessage::Progress(data) => {
                if data.prompt_id == self.prompt_id
                    && data.node == self.prompt.k_sampler_node_id.as_str()
                {
                    let mut result = self.result.write().await;

                    let mut current_running_result = match &*result {
                        WorkflowResult::Running(result) => result.clone(),
                        _ => WorkflowRunningResult {
                            progress: 0.0,
                            previews: vec![],
                        },
                    };

                    current_running_result.progress = (data.value as f32) / (data.max as f32);
                    *result = WorkflowResult::Running(current_running_result);
                }
            }
            WorkflowMessage::ExecutionSuccess(data) => {
                if data.prompt_id == self.prompt_id {
                    tracing::info!("execution success: {:?}", data);
                    let mut result = self.result.write().await;
                    // Move out the items and reset the field
                    *result = WorkflowResult::Done(std::mem::take(&mut self.results));

                    return true;
                }
            }
            WorkflowMessage::ExecutionError(error) => {
                if error.prompt_id == self.prompt_id {
                    tracing::info!("execution error: {:?}", error);
                    let mut result = self.result.write().await;
                    *result = WorkflowResult::Error(error.exception_message.unwrap_or_default());

                    return true;
                }
            }
        }

        false
    }

    async fn on_binary(&mut self, data: Vec<u8>) {
        // this is preview image or final results
        if let Some(current_node_id) = &self.current_node_id {
            if current_node_id == self.prompt.k_sampler_node_id.as_str() {
                let mut result = self.result.write().await;
                if let WorkflowResult::Running(result) = &mut *result {
                    result.previews = vec![data[8..].to_vec()];
                }
            } else if current_node_id == self.prompt.output_node_id.as_str() {
                self.results.push(data[8..].to_vec());
            }
        }
    }

    /// Establish websocket connection with ComfyUI,
    /// and update result when new message come in.
    pub async fn run(&mut self, node: &Url) -> Result<(), WorkflowExecutionError> {
        let mut connection_url = node.join("/ws").expect("");
        let _ = connection_url.set_scheme("ws");
        connection_url.set_query(Some(&format!("clientId={}", &self.task_id)));

        let (mut ws_stream, _) = connect_async(&connection_url.to_string())
            .await
            .map_err(|_| WorkflowExecutionError::WebSocketConnectionError)?;

        tracing::info!("trigger workflow");

        let prompt_id = self.trigger_workflow(node).await?;
        self.prompt_id = prompt_id;

        tracing::info!("prompt_id: {}, node: {}", &self.prompt_id, node);

        while let Some(msg) = ws_stream.next().await {
            match msg {
                Ok(msg) => {
                    match msg {
                        Message::Text(text) => {
                            match serde_json::from_str::<WorkflowMessage>(&text) {
                                Ok(data) => {
                                    if self.on_message(data).await {
                                        break;
                                    }
                                }
                                _ => {
                                    tracing::warn!("unknown message: {}", text);
                                }
                            }
                        }
                        Message::Binary(data) => {
                            self.on_binary(data).await;
                        }
                        _ => {
                            // safely ignore other types
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("websocket error: {}", e);
                }
            }
        }

        Ok(())
    }
}
