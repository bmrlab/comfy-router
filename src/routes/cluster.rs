use super::{AppError, AppJson};
use crate::{
    cluster::{NodeState, NodeStatus},
    state::AppState,
    workflow::record::run_task,
};
use axum::{
    extract::State,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use url::Url;
use utoipa::ToSchema;

const OPENAPI_TAG: &str = "Cluster";

#[cfg(not(debug_assertions))]
use reqwest::{Client, StatusCode};

#[cfg(not(debug_assertions))]
use std::{collections::HashMap, time::Duration};

#[cfg(not(debug_assertions))]
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

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct RequestUrl {
    #[schema(value_type = String)]
    url: Url,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct NodeResponse {
    #[schema(value_type = String)]
    pub url: Url,
    pub status: NodeStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct NodesResponse {
    nodes: Vec<NodeResponse>,
}

/// Add node
/// 
/// Add a single ComfyUI node to cluster using URL.
#[utoipa::path(
    post,
    path = "/cluster/nodes",
    request_body = RequestUrl,
    responses((
        status = OK, description = "Add node successfully.", body = (), 
    )),
    security(("basic_auth" = [])),
    tag = OPENAPI_TAG
)]
pub async fn join(
    State(state): State<Arc<AppState>>,
    AppJson(data): AppJson<RequestUrl>,
) -> Result<AppJson<()>, AppError> {
    let node_state = state.node_state();
    if node_state.read().await.get(&data.url).is_none() {
        let mut node_state = node_state.write().await;
        node_state.add(&data.url);
    }

    // after new node join, safely trigger new task to run
    tokio::spawn(async move {
        run_task(state).await;
    });

    Ok(AppJson(()))
}

/// Remove node
/// 
/// Remove a node from cluster using URL.
#[utoipa::path(
    post,
    path = "/cluster/nodes/delete",
    request_body = RequestUrl,
    responses((
        status = OK, description = "Remove node successfully.", body = (), 
    )),
    security(("basic_auth" = [])),
    tag = OPENAPI_TAG
)]
pub async fn remove(
    State(state): State<Arc<AppState>>,
    AppJson(data): AppJson<RequestUrl>,
) -> Result<AppJson<()>, AppError> {
    let node_state = state.node_state();
    let mut node_state = node_state.write().await;
    node_state.remove(&data.url);

    Ok(AppJson(()))
}

/// List nodes
/// 
/// List all nodes in cluster.
#[utoipa::path(
    get,
    path = "/cluster/nodes",
    responses((
        status = OK, body = NodesResponse,
    )),
    security(("basic_auth" = [])),
    tag = OPENAPI_TAG
)]
pub async fn nodes(State(state): State<Arc<AppState>>) -> Result<AppJson<NodesResponse>, AppError> {
    let node_state = state.node_state();
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

#[allow(unused_variables)]
pub fn cluster_routes(node_state: Arc<RwLock<NodeState>>) -> Router<Arc<AppState>> {
    #[cfg(not(debug_assertions))]
    tokio::spawn(async move {
        let mut node_unhealthy_count = HashMap::new();
        loop {
            health_check(node_state.clone(), &mut node_unhealthy_count).await;
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });

    Router::new()
        .route("/nodes", post(join))
        .route("/nodes", get(nodes))
        .route("/nodes/delete", post(remove))
}
