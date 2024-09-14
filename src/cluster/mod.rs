use serde::{Deserialize, Serialize};
use std::collections::{hash_map::Iter, HashMap};
use url::Url;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Status {
    Idle,
    Busy,
    Offline,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeStatus {
    status: Status,
    cache: HashMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeState {
    nodes: HashMap<Url, NodeStatus>,
    task_record: HashMap<String, Url>,
}

impl Default for NodeStatus {
    fn default() -> Self {
        Self {
            status: Status::Idle,
            cache: HashMap::new(),
        }
    }
}

impl NodeState {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            task_record: HashMap::new(),
        }
    }

    pub fn add(&mut self, url: &Url) {
        self.nodes.insert(url.clone(), NodeStatus::default());
    }

    pub fn get(&self, url: &Url) -> Option<&NodeStatus> {
        self.nodes.get(url)
    }

    pub fn get_all<'a>(&'a self) -> Iter<'a, Url, NodeStatus> {
        self.nodes.iter()
    }

    pub fn remove(&mut self, url: &Url) {
        if let Some(status) = self.nodes.get(url) {
            if status.status != Status::Busy {
                self.nodes.remove(url);
            }
        }
    }

    pub fn pick(&mut self, target: &HashMap<String, String>) -> Option<Url> {
        let picked = self
            .nodes
            .iter()
            .find(|v| v.1.status == Status::Idle)
            .map(|v| v.0.clone());

        if let Some(url) = &picked {
            self.set_busy(url);
            self.set_cache(url, target);
        }

        picked
    }

    pub fn set_busy(&mut self, url: &Url) {
        if let Some(status) = self.nodes.get_mut(url) {
            status.status = Status::Busy;
        }
    }

    pub fn set_idle(&mut self, url: &Url) {
        if let Some(status) = self.nodes.get_mut(url) {
            status.status = Status::Idle;
        }
    }

    pub fn set_offline(&mut self, url: &Url) {
        if let Some(status) = self.nodes.get_mut(url) {
            status.status = Status::Offline;
        }
    }

    pub fn set_cache(&mut self, url: &Url, cache: &HashMap<String, String>) {
        if let Some(status) = self.nodes.get_mut(url) {
            status.cache = cache.clone();
        }
    }
}
