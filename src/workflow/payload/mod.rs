pub mod flux;
pub mod sd15;
pub mod sdxl;

use super::fetch::{Fetch, FetchHelper};
use crate::{
    download::{create_download_task, task::DownloadStatus, CreateDownloadTaskResult},
    state::AppState,
};
use flux::FluxWorkflowPayload;
use sd15::SD15WorkflowPayload;
use sdxl::SDXLWorkflowPayload;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::watch;
use url::Url;
use utoipa::ToSchema;

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case", tag = "type", content = "name")]
pub enum Model {
    BuildIn(String),
    #[schema(value_type = String, default = String::default)]
    Custom(Url),
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case", tag = "type", content = "content")]
pub enum Image {
    /// not support for now
    Base64(String),
    #[schema(value_type = String, default = String::default)]
    Url(Url),
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct ControlNetPayload {
    model: Model,
    weight: f32,
    start_at: f32,
    end_at: f32,
    preprocessor: Option<String>,
    image: Image,
    resize_mode: String,
    preprocessor_params: Option<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct LoRAPayload {
    model: Model,
    weight: f32,
}

#[derive(Clone, Debug)]
pub struct ComfyUIPrompt {
    pub prompt: Value,
    pub k_sampler_node_id: String,
    pub output_node_id: String,
}

pub struct CurrentNodeId {
    inner: u32,
}

impl CurrentNodeId {
    fn new() -> Self {
        Self { inner: 2 }
    }

    fn get(&mut self) -> String {
        self.inner += 1;
        self.inner.to_string()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", content = "params")]
pub enum WorkflowPayload {
    SD15(SD15WorkflowPayload),
    SDXL(SDXLWorkflowPayload),
    Flux(FluxWorkflowPayload),
}

impl WorkflowPayload {
    pub fn cache_map(&self) -> HashMap<String, String> {
        HashMap::new()
    }
}

impl Fetch for &Model {
    async fn fetch(
        &self,
        app_state: Arc<AppState>,
        target_folder: &str,
    ) -> (String, Option<watch::Receiver<DownloadStatus>>) {
        match self {
            Model::BuildIn(name) => (name.to_string(), None),
            Model::Custom(url) => {
                let (file_name, result) =
                    create_download_task(&url, target_folder, app_state.download_state()).await;

                match result {
                    CreateDownloadTaskResult::Existed => (file_name, None),
                    CreateDownloadTaskResult::Created(rx) => (file_name, Some(rx)),
                }
            }
        }
    }
}

impl Fetch for &Image {
    async fn fetch(
        &self,
        app_state: Arc<AppState>,
        target_folder: &str,
    ) -> (String, Option<watch::Receiver<DownloadStatus>>) {
        match self {
            Image::Url(url) => {
                let (file_name, result) =
                    create_download_task(&url, target_folder, app_state.download_state()).await;

                match result {
                    CreateDownloadTaskResult::Existed => (file_name, None),
                    CreateDownloadTaskResult::Created(rx) => (file_name, Some(rx)),
                }
            }
            _ => {
                todo!()
            }
        }
    }
}

pub async fn generate_comfy_prompt(
    payload: &WorkflowPayload,
    app_state: Arc<AppState>,
) -> anyhow::Result<ComfyUIPrompt> {
    let fetch_helper = FetchHelper::new(app_state.clone());

    match payload {
        WorkflowPayload::SD15(payload) => payload.into_comfy_prompt(fetch_helper).await,
        WorkflowPayload::Flux(payload) => payload.into_comfy_prompt(fetch_helper).await,
        _ => {
            todo!()
        }
    }
}
