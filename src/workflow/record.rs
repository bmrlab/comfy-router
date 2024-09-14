use super::{payload::WorkflowPayload, task::WorkflowTask};
use crate::state::AppState;
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};
use thiserror::Error;

#[derive(Clone, Debug)]
pub struct WorkflowRecord {
    inner: HashMap<String, WorkflowTask>,
    capacity: usize,
    order: VecDeque<String>,
    pending: VecDeque<String>,
    pending_capacity: usize,
}

#[derive(Error, Debug)]
pub enum WorkflowRecordError {
    #[error("pending queue is full")]
    PendingQueueFull,
}

impl WorkflowRecord {
    pub fn new(capacity: usize, pending_capacity: usize) -> Self {
        Self {
            inner: HashMap::new(),
            capacity,
            order: VecDeque::new(),
            pending: VecDeque::new(),
            pending_capacity,
        }
    }

    pub fn add(&mut self, payload: WorkflowPayload) -> Result<&WorkflowTask, WorkflowRecordError> {
        let task = WorkflowTask::new(payload);
        let task_id = task.id().to_string();

        if self.pending.len() == self.pending_capacity {
            return Err(WorkflowRecordError::PendingQueueFull);
        }

        self.pending.push_back(task_id.clone());

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

        Ok(self.get(&task_id).expect("task_id should exist"))
    }

    pub fn get(&self, id: &str) -> Option<&WorkflowTask> {
        self.inner.get(id)
    }

    pub fn pop_pending(&mut self) -> Option<&WorkflowTask> {
        match self.pending.pop_front() {
            Some(task_id) => self.get(&task_id),
            None => None,
        }
    }

    pub fn top_pending(&self) -> Option<&WorkflowTask> {
        self.pending.front().and_then(|k| self.get(k))
    }
}

pub async fn run_task(app_state: Arc<AppState>) {
    loop {
        let task_payload = {
            let workflow_record = app_state.workflow_record();
            let workflow_record = workflow_record.read().await;
            workflow_record.top_pending().map(|v| v.payload().clone())
        };

        if let Some(payload) = task_payload {
            let picked = {
                let cache_map = payload.cache_map();
                let node_state = app_state.node_state();
                let mut node_state = node_state.write().await;
                node_state.pick(&cache_map)
            };

            if let Some(node) = picked {
                let task = {
                    let workflow_record = app_state.workflow_record();
                    let mut workflow_record = workflow_record.write().await;
                    workflow_record.pop_pending().map(|v| v.clone())
                }
                .expect("task should exist");

                task.run(&node, app_state.clone()).await;

                // after task done, set node to idle
                {
                    let node_state = app_state.node_state();
                    let mut node_state = node_state.write().await;
                    node_state.set_idle(&node);
                }
            } else {
                break;
            }
        } else {
            break;
        }
    }
}
