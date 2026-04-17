//! `vscode.tests` (Test Explorer) API compatibility shim.
//!
//! Provides the VS Code Testing API for registering test controllers,
//! managing test items, running/debugging tests, and reporting results.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, RwLock};

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Opaque handles
// ---------------------------------------------------------------------------

/// Opaque handle to a test controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TestControllerId(pub u32);

/// Opaque handle to a test run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TestRunId(pub u32);

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// A test item (mirrors `vscode.TestItem`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestItem {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub uri: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub sort_text: Option<String>,
    #[serde(default)]
    pub tags: Vec<TestTag>,
    #[serde(default)]
    pub range: Option<TestRange>,
    #[serde(default)]
    pub can_resolve_children: bool,
    #[serde(default)]
    pub busy: bool,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub children: Vec<TestItem>,
}

/// A tag on a test item or run profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestTag {
    pub id: String,
}

/// Line/column range for a test item in source.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestRange {
    pub start_line: u32,
    pub start_character: u32,
    pub end_line: u32,
    pub end_character: u32,
}

/// Result state for a test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TestResultState {
    Queued,
    Running,
    Passed,
    Failed,
    Skipped,
    Errored,
}

/// A message attached to a test result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestMessage {
    pub message: String,
    #[serde(default)]
    pub expected_output: Option<String>,
    #[serde(default)]
    pub actual_output: Option<String>,
    #[serde(default)]
    pub location: Option<TestMessageLocation>,
    #[serde(default)]
    pub context_value: Option<String>,
}

/// Source location for a test message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestMessageLocation {
    pub uri: String,
    pub range: TestRange,
}

/// Kind of test run profile (Run, Debug, Coverage).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TestRunProfileKind {
    Run = 1,
    Debug = 2,
    Coverage = 3,
}

/// A test run profile (e.g. "Run Tests", "Debug Tests").
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestRunProfile {
    pub label: String,
    pub kind: TestRunProfileKind,
    #[serde(default)]
    pub is_default: bool,
    #[serde(default)]
    pub tag: Option<TestTag>,
    #[serde(default)]
    pub supports_continuous_run: bool,
}

/// An individual test result recorded by a test run.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestRunResult {
    pub test_id: String,
    pub state: TestResultState,
    #[serde(default)]
    pub duration_ms: Option<f64>,
    #[serde(default)]
    pub messages: Vec<TestMessage>,
}

// ---------------------------------------------------------------------------
// Callback types
// ---------------------------------------------------------------------------

/// Handler invoked when a test run is requested.
pub type TestRunHandler =
    Arc<dyn Fn(&[String], TestRunProfileKind) -> Result<Vec<TestRunResult>> + Send + Sync>;

/// Callback for test events.
pub type TestEventListener = Arc<dyn Fn(Value) -> Result<()> + Send + Sync>;

// ---------------------------------------------------------------------------
// Internal state
// ---------------------------------------------------------------------------

#[allow(dead_code)]
struct TestControllerEntry {
    id: TestControllerId,
    controller_id: String,
    label: String,
    items: HashMap<String, TestItem>,
    profiles: Vec<TestRunProfile>,
    run_handler: Option<TestRunHandler>,
}

#[allow(dead_code)]
struct TestRunEntry {
    id: TestRunId,
    controller_id: TestControllerId,
    profile_kind: TestRunProfileKind,
    results: Vec<TestRunResult>,
    is_running: bool,
}

// ---------------------------------------------------------------------------
// TestApi
// ---------------------------------------------------------------------------

/// Implements the `vscode.tests.*` (Test Explorer) API surface.
pub struct TestApi {
    next_controller: AtomicU32,
    next_run: AtomicU32,

    controllers: RwLock<HashMap<TestControllerId, TestControllerEntry>>,
    runs: RwLock<HashMap<TestRunId, TestRunEntry>>,
}

impl TestApi {
    /// Creates a new test API handler.
    pub fn new() -> Self {
        Self {
            next_controller: AtomicU32::new(1),
            next_run: AtomicU32::new(1),
            controllers: RwLock::new(HashMap::new()),
            runs: RwLock::new(HashMap::new()),
        }
    }

    /// Dispatches a test API action.
    pub fn handle(&self, action: &str, params: &Value) -> Result<Value> {
        match action {
            "createTestController" => {
                let id = params.get("id").and_then(Value::as_str).unwrap_or("");
                let label = params.get("label").and_then(Value::as_str).unwrap_or("");
                let ctrl_id = self.create_test_controller(id, label);
                Ok(serde_json::to_value(ctrl_id)?)
            }
            "addTestItem" => {
                let ctrl_id = params
                    .get("controllerId")
                    .and_then(Value::as_u64)
                    .map(|n| TestControllerId(u32::try_from(n).unwrap_or(0)))
                    .ok_or_else(|| anyhow::anyhow!("missing controllerId"))?;
                let item: TestItem =
                    serde_json::from_value(params.get("item").cloned().unwrap_or(Value::Null))?;
                self.add_test_item(ctrl_id, item)?;
                Ok(Value::Bool(true))
            }
            "removeTestItem" => {
                let ctrl_id = params
                    .get("controllerId")
                    .and_then(Value::as_u64)
                    .map(|n| TestControllerId(u32::try_from(n).unwrap_or(0)))
                    .ok_or_else(|| anyhow::anyhow!("missing controllerId"))?;
                let item_id = params.get("itemId").and_then(Value::as_str).unwrap_or("");
                self.remove_test_item(ctrl_id, item_id)?;
                Ok(Value::Bool(true))
            }
            "addTestRunProfile" => {
                let ctrl_id = params
                    .get("controllerId")
                    .and_then(Value::as_u64)
                    .map(|n| TestControllerId(u32::try_from(n).unwrap_or(0)))
                    .ok_or_else(|| anyhow::anyhow!("missing controllerId"))?;
                let profile: TestRunProfile =
                    serde_json::from_value(params.get("profile").cloned().unwrap_or(Value::Null))?;
                self.add_test_run_profile(ctrl_id, profile)?;
                Ok(Value::Bool(true))
            }
            "createTestRun" => {
                let ctrl_id = params
                    .get("controllerId")
                    .and_then(Value::as_u64)
                    .map(|n| TestControllerId(u32::try_from(n).unwrap_or(0)))
                    .ok_or_else(|| anyhow::anyhow!("missing controllerId"))?;
                let kind: TestRunProfileKind = params
                    .get("profileKind")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or(TestRunProfileKind::Run);
                let run_id = self.create_test_run(ctrl_id, kind)?;
                Ok(serde_json::to_value(run_id)?)
            }
            "reportTestResult" => {
                let run_id = params
                    .get("runId")
                    .and_then(Value::as_u64)
                    .map(|n| TestRunId(u32::try_from(n).unwrap_or(0)))
                    .ok_or_else(|| anyhow::anyhow!("missing runId"))?;
                let result: TestRunResult =
                    serde_json::from_value(params.get("result").cloned().unwrap_or(Value::Null))?;
                self.report_test_result(run_id, result)?;
                Ok(Value::Bool(true))
            }
            "endTestRun" => {
                let run_id = params
                    .get("runId")
                    .and_then(Value::as_u64)
                    .map(|n| TestRunId(u32::try_from(n).unwrap_or(0)))
                    .ok_or_else(|| anyhow::anyhow!("missing runId"))?;
                self.end_test_run(run_id)?;
                Ok(Value::Bool(true))
            }
            "dispose" => {
                let ctrl_id = params
                    .get("controllerId")
                    .and_then(Value::as_u64)
                    .map(|n| TestControllerId(u32::try_from(n).unwrap_or(0)))
                    .ok_or_else(|| anyhow::anyhow!("missing controllerId"))?;
                self.dispose(ctrl_id);
                Ok(Value::Bool(true))
            }
            _ => bail!("unknown test action: {action}"),
        }
    }

    // -----------------------------------------------------------------------
    // Controllers
    // -----------------------------------------------------------------------

    /// Creates a test controller.
    pub fn create_test_controller(&self, controller_id: &str, label: &str) -> TestControllerId {
        let raw = self.next_controller.fetch_add(1, Ordering::Relaxed);
        let id = TestControllerId(raw);
        log::debug!("[ext] createTestController({controller_id}, {label}) -> {raw}");
        self.controllers
            .write()
            .expect("test controllers lock poisoned")
            .insert(
                id,
                TestControllerEntry {
                    id,
                    controller_id: controller_id.to_owned(),
                    label: label.to_owned(),
                    items: HashMap::new(),
                    profiles: Vec::new(),
                    run_handler: None,
                },
            );
        id
    }

    /// Sets the run handler on a test controller.
    pub fn set_run_handler(
        &self,
        ctrl_id: TestControllerId,
        handler: TestRunHandler,
    ) -> Result<()> {
        let mut controllers = self
            .controllers
            .write()
            .expect("test controllers lock poisoned");
        let ctrl = controllers
            .get_mut(&ctrl_id)
            .ok_or_else(|| anyhow::anyhow!("test controller {} not found", ctrl_id.0))?;
        ctrl.run_handler = Some(handler);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Test items
    // -----------------------------------------------------------------------

    /// Adds a test item to a controller.
    pub fn add_test_item(&self, ctrl_id: TestControllerId, item: TestItem) -> Result<()> {
        let mut controllers = self
            .controllers
            .write()
            .expect("test controllers lock poisoned");
        let ctrl = controllers
            .get_mut(&ctrl_id)
            .ok_or_else(|| anyhow::anyhow!("test controller {} not found", ctrl_id.0))?;
        ctrl.items.insert(item.id.clone(), item);
        Ok(())
    }

    /// Removes a test item from a controller.
    pub fn remove_test_item(&self, ctrl_id: TestControllerId, item_id: &str) -> Result<()> {
        let mut controllers = self
            .controllers
            .write()
            .expect("test controllers lock poisoned");
        let ctrl = controllers
            .get_mut(&ctrl_id)
            .ok_or_else(|| anyhow::anyhow!("test controller {} not found", ctrl_id.0))?;
        ctrl.items.remove(item_id);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Run profiles
    // -----------------------------------------------------------------------

    /// Adds a test run profile to a controller.
    pub fn add_test_run_profile(
        &self,
        ctrl_id: TestControllerId,
        profile: TestRunProfile,
    ) -> Result<()> {
        let mut controllers = self
            .controllers
            .write()
            .expect("test controllers lock poisoned");
        let ctrl = controllers
            .get_mut(&ctrl_id)
            .ok_or_else(|| anyhow::anyhow!("test controller {} not found", ctrl_id.0))?;
        ctrl.profiles.push(profile);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Test runs
    // -----------------------------------------------------------------------

    /// Creates a test run.
    pub fn create_test_run(
        &self,
        controller_id: TestControllerId,
        profile_kind: TestRunProfileKind,
    ) -> Result<TestRunId> {
        let raw = self.next_run.fetch_add(1, Ordering::Relaxed);
        let id = TestRunId(raw);
        log::debug!(
            "[ext] createTestRun(controller={}, kind={:?}) -> {raw}",
            controller_id.0,
            profile_kind,
        );
        self.runs.write().expect("test runs lock poisoned").insert(
            id,
            TestRunEntry {
                id,
                controller_id,
                profile_kind,
                results: Vec::new(),
                is_running: true,
            },
        );
        Ok(id)
    }

    /// Reports a result for a test within a run.
    pub fn report_test_result(&self, run_id: TestRunId, result: TestRunResult) -> Result<()> {
        let mut runs = self.runs.write().expect("test runs lock poisoned");
        let run = runs
            .get_mut(&run_id)
            .ok_or_else(|| anyhow::anyhow!("test run {} not found", run_id.0))?;
        run.results.push(result);
        Ok(())
    }

    /// Ends a test run.
    pub fn end_test_run(&self, run_id: TestRunId) -> Result<()> {
        let mut runs = self.runs.write().expect("test runs lock poisoned");
        if let Some(run) = runs.get_mut(&run_id) {
            run.is_running = false;
            log::debug!("[ext] endTestRun({})", run_id.0);
        }
        Ok(())
    }

    /// Disposes a test controller and its associated runs.
    pub fn dispose(&self, ctrl_id: TestControllerId) {
        self.controllers
            .write()
            .expect("test controllers lock poisoned")
            .remove(&ctrl_id);

        self.runs
            .write()
            .expect("test runs lock poisoned")
            .retain(|_, r| r.controller_id != ctrl_id);
    }
}

impl Default for TestApi {
    fn default() -> Self {
        Self::new()
    }
}
