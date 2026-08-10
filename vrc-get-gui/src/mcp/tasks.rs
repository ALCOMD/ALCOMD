use rmcp::{
    ErrorData as McpError,
    model::{DetailedTask, Task},
    task_manager::{TaskContext, TaskFuture, TaskManager, TaskOptions},
};
use serde_json::Value;

/// Process-wide MCP task state. Transport handlers receive clones of this
/// handle so listener restarts never replace the underlying task registry.
#[derive(Clone, Default)]
pub(super) struct McpTaskManager {
    inner: TaskManager,
}

impl McpTaskManager {
    pub(super) fn new() -> Self {
        Self {
            inner: TaskManager::new(),
        }
    }

    pub(super) fn spawn<F>(&self, options: TaskOptions, make_future: F) -> Task
    where
        F: FnOnce(TaskContext) -> TaskFuture,
    {
        self.inner.spawn(options, make_future)
    }

    pub(super) fn get_task(&self, task_id: &str) -> Result<DetailedTask, McpError> {
        self.inner.get_task(task_id)
    }

    pub(super) fn update_task<I>(&self, task_id: &str, input_responses: I) -> Result<(), McpError>
    where
        I: IntoIterator<Item = (String, Value)>,
    {
        self.inner.update_task(task_id, input_responses)
    }

    pub(super) fn cancel_task(&self, task_id: &str) -> Result<(), McpError> {
        self.inner.cancel_task(task_id)
    }

    #[cfg(test)]
    pub(super) fn running_task_count(&self) -> usize {
        self.inner.running_task_count()
    }

    pub(super) fn shutdown(&self) {
        self.inner.shutdown();
    }
}
