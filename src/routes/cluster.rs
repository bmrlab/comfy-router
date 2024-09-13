use super::{AppError, AppJson};
use crate::{
    cluster::{NodeState, NodeStatus},
    state::AppState,
};
use axum::{
    routing::{get, post},
    Extension, Router,
};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::RwLock;
use tower::ServiceBuilder;
use tower_http::add_extension::AddExtensionLayer;
use url::Url;

async fn health_check(
    node_state: Arc<RwLock<NodeState>>,
    unhealthy_count: &mut HashMap<Url, usize>,
) {
    let node_urls: Vec<Url> = {
        let node_state = node_state.read().await;
        node_state.get_all().map(|(k, _)| k.clone()).collect()
    };

    if node_urls.is_empty() {
        return;
    }

    let client = Client::new();
    for node_url in node_urls {
        let resp = client.get(node_url.join("/prompt").expect("")).send().await;
        match resp {
            Ok(resp) if resp.status() == StatusCode::OK => {
                match unhealthy_count.remove(&node_url) {
                    Some(count) => {
                        if count >= 3 {
                            let mut node_state = node_state.write().await;
                            node_state.set_idle(&node_url);
                        }
                    }
                    _ => {}
                }
            }
            _ => {
                tracing::warn!("node {} is unhealthy", node_url);

                match unhealthy_count.get(&node_url) {
                    Some(count) => {
                        let new_count = count + 1;
                        unhealthy_count.insert(node_url.clone(), new_count);

                        if new_count >= 3 {
                            let mut node_state = node_state.write().await;
                            node_state.set_offline(&node_url);
                        }
                    }
                    _ => {
                        unhealthy_count.insert(node_url, 1);
                    }
                }
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RequestUrl {
    url: Url,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeResponse {
    pub url: Url,
    pub status: NodeStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodesResponse {
    nodes: Vec<NodeResponse>,
}

pub async fn join(
    Extension(node_state): Extension<Arc<RwLock<NodeState>>>,
    AppJson(data): AppJson<RequestUrl>,
) -> Result<AppJson<()>, AppError> {
    if node_state.read().await.get(&data.url).is_none() {
        let mut node_state = node_state.write().await;
        node_state.add(&data.url);
    }

    Ok(AppJson(()))
}

pub async fn nodes(
    Extension(node_state): Extension<Arc<RwLock<NodeState>>>,
) -> Result<AppJson<NodesResponse>, AppError> {
    let node_state = node_state.read().await;

    Ok(AppJson(NodesResponse {
        nodes: node_state
            .get_all()
            .map(|(url, status)| NodeResponse {
                url: url.clone(),
                status: status.clone(),
            })
            .collect(),
    }))
}

pub fn cluster_routes(node_state: Arc<RwLock<NodeState>>) -> Router<Arc<AppState>> {
    let node_state_clone = node_state.clone();

    tokio::spawn(async move {
        let mut node_unhealthy_count = HashMap::new();
        loop {
            health_check(node_state_clone.clone(), &mut node_unhealthy_count).await;
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });

    Router::new()
        .route("/join", post(join))
        .route("/nodes", get(nodes))
        .layer(
            ServiceBuilder::new()
                .layer(AddExtensionLayer::new(node_state))
                .into_inner(),
        )
}
