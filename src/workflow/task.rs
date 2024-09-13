use crate::{download::state::DownloadState, state::AppState};

use super::{
    message::WorkflowMessage,
    payload::{ComfyUIPrompt, WorkflowPayload},
};
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};
use tokio::sync::RwLock;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use url::Url;

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
    node: Url,
    payload: WorkflowPayload,
    result: Arc<RwLock<WorkflowResult>>,
}

#[derive(Clone, Debug)]
pub struct WorkflowRecord {
    inner: HashMap<String, WorkflowTask>,
    capacity: usize,
    order: VecDeque<String>,
}

impl WorkflowRecord {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: HashMap::new(),
            capacity,
            order: VecDeque::new(),
        }
    }

    pub fn add(&mut self, task: WorkflowTask) {
        let task_id = task.id().to_string();
        if self.inner.contains_key(&task_id) {
            self.order.retain(|k| k != task_id.as_str());
        }

        if self.inner.len() == self.capacity {
            if let Some(oldest_key) = self.order.pop_front() {
                self.inner.remove(&oldest_key);
            }
        }

        self.inner.insert(task_id.clone(), task);
        self.order.push_back(task_id.clone());
    }

    pub fn get(&self, id: &str) -> Option<&WorkflowTask> {
        self.inner.get(id)
    }
}

impl WorkflowTask {
    pub fn new(url: &Url, payload: WorkflowPayload) -> Self {
        let result = Arc::new(RwLock::new(WorkflowResult::Pending(0)));

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            node: url.clone(),
            payload,
            result,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub async fn result(&self) -> WorkflowResult {
        self.result.read().await.clone()
    }

    pub fn node(&self) -> &Url {
        &self.node
    }

    async fn trigger_workflow(&self, prompt: &ComfyUIPrompt) -> anyhow::Result<String> {
        let client = Client::new();
        let response = client
            .post(self.node.join("/prompt").expect(""))
            .json(&json!({
                "prompt": &prompt.prompt,
                "client_id": &self.id
            }))
            .send()
            .await?;

        let response_json = response.json::<Value>().await?;
        tracing::debug!("response: {:?}", response_json);
        let response_json = response_json
            .as_object()
            .ok_or(anyhow::anyhow!("invalid response: {:?}", response_json))?;
        let prompt_id = response_json
            .get("prompt_id")
            .ok_or(anyhow::anyhow!("cannot find prompt_id: {:?}", response_json))?;
        let prompt_id = prompt_id
            .as_str()
            .ok_or(anyhow::anyhow!("invalid prompt_id: {:?}", response_json))?;

        Ok(prompt_id.to_string())
    }

    #[tracing::instrument(skip_all, fields(task_id = self.id))]
    pub async fn run(&self, download_state: Arc<RwLock<DownloadState>>, app_state: Arc<AppState>) {
        let prompt = self
            .payload
            .into_comfy_prompt(download_state, app_state)
            .await;

        tracing::info!("got prompt");
        // establish websocket connection with ComfyUI
        // and update result when new message come in
        let mut connection_url = self.node.clone().join("/ws").expect("");
        let _ = connection_url.set_scheme("ws");
        connection_url.set_query(Some(&format!("clientId={}", &self.id)));

        if let Ok((mut ws_stream, _)) = connect_async(&connection_url.to_string()).await {
            tracing::debug!("task websocket connected");
            tracing::info!("trigger workflow");

            match self.trigger_workflow(&prompt).await {
                Ok(prompt_id) => {
                    tracing::info!("prompt_id: {}, node: {}", prompt_id, &self.node);

                    let mut current_node_id: Option<String> = None;
                    let k_sampler_node_id = prompt.k_sampler_node_id.as_str();
                    let output_node_id = prompt.output_node_id.as_str();
                    let mut results = vec![];

                    while let Some(msg) = ws_stream.next().await {
                        match msg {
                            Ok(msg) => {
                                match msg {
                                    Message::Text(text) => {
                                        match serde_json::from_str::<WorkflowMessage>(&text) {
                                            Ok(data) => {
                                                match data {
                                                    WorkflowMessage::Status(_) => {
                                                        // safely ignore
                                                    }
                                                    WorkflowMessage::ExecutionStart(data) => {
                                                        if data.prompt_id == prompt_id {
                                                            tracing::debug!(
                                                                "execution start: {:?}",
                                                                data
                                                            );
                                                            // update task status
                                                            let mut result =
                                                                self.result.write().await;

                                                            *result = WorkflowResult::Running(
                                                                WorkflowRunningResult {
                                                                    progress: 0.0,
                                                                    previews: vec![],
                                                                },
                                                            );
                                                        }
                                                    }
                                                    WorkflowMessage::ExecutionCached(_) => {
                                                        // safely ignore
                                                    }
                                                    WorkflowMessage::Executing(data) => {
                                                        if data.prompt_id == prompt_id {
                                                            current_node_id = Some(data.node);
                                                        }
                                                    }
                                                    WorkflowMessage::Progress(data) => {
                                                        if data.prompt_id == prompt_id
                                                            && data.node == k_sampler_node_id
                                                        {
                                                            let mut result =
                                                                self.result.write().await;

                                                            let mut current_running_result =
                                                                match &*result {
                                                                    WorkflowResult::Running(
                                                                        result,
                                                                    ) => result.clone(),
                                                                    _ => WorkflowRunningResult {
                                                                        progress: 0.0,
                                                                        previews: vec![],
                                                                    },
                                                                };

                                                            current_running_result.progress =
                                                                (data.value as f32)
                                                                    / (data.max as f32);

                                                            *result = WorkflowResult::Running(
                                                                current_running_result,
                                                            );
                                                        }
                                                    }
                                                    WorkflowMessage::ExecutionSuccess(data) => {
                                                        if data.prompt_id == prompt_id {
                                                            tracing::info!(
                                                                "execution success: {:?}",
                                                                data
                                                            );
                                                            let mut result =
                                                                self.result.write().await;
                                                            *result = WorkflowResult::Done(results);
                                                            break;
                                                        }
                                                    }
                                                    WorkflowMessage::ExecutionError(error) => {
                                                        if error.prompt_id == prompt_id {
                                                            tracing::info!(
                                                                "execution error: {:?}",
                                                                error
                                                            );
                                                            let mut result =
                                                                self.result.write().await;
                                                            *result = WorkflowResult::Error(
                                                                error
                                                                    .exception_message
                                                                    .unwrap_or_default(),
                                                            );
                                                            break;
                                                        }
                                                    }
                                                }
                                            }
                                            _ => {
                                                tracing::warn!("unknown message: {}", text);
                                            }
                                        }
                                    }
                                    Message::Binary(data) => {
                                        // this is preview image or final results
                                        if let Some(current_node_id) = &current_node_id {
                                            if current_node_id == k_sampler_node_id {
                                                let mut result = self.result.write().await;
                                                if let WorkflowResult::Running(result) =
                                                    &mut *result
                                                {
                                                    result.previews = vec![data[8..].to_vec()];
                                                }
                                            } else if current_node_id == output_node_id {
                                                results.push(data[8..].to_vec());
                                            }
                                        }
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
                }
                Err(e) => {
                    tracing::warn!("failed to trigger workflow: {}", e);

                    let mut result = self.result.write().await;
                    *result = WorkflowResult::Error(format!("failed to trigger workflow: {}", e));
                }
            };
        } else {
            tracing::warn!("failed to connect to websocket");
            let mut result = self.result.write().await;
            *result = WorkflowResult::Error("failed to connect to websocket".to_string());
        }
    }
}
