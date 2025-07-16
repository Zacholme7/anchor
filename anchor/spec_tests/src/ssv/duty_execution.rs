use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::message_processing::{RunnerConfig, ValidatorDuty};
use crate::{SpecTest, SpecTestType, SsvSpecTestType};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SsvDutyExecutionTest {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Type")]
    pub test_type: String,
    #[serde(rename = "Documentation")]
    pub documentation: String,
    #[serde(rename = "Tests")]
    pub tests: Vec<DutyExecutionSubTest>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DutyExecutionSubTest {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Runner")]
    pub runner: RunnerConfig,
    #[serde(rename = "Duty")]
    pub duty: ValidatorDuty,
    #[serde(rename = "PostDutyRunnerStateRoot")]
    pub post_duty_runner_state_root: String,
    #[serde(rename = "PostDutyRunnerState")]
    pub post_duty_runner_state: Option<Value>,
    #[serde(rename = "OutputMessages")]
    pub output_messages: Vec<Value>,
    #[serde(rename = "BeaconBroadcastedRoots")]
    pub beacon_broadcasted_roots: Vec<String>,
    #[serde(rename = "ExpectedError")]
    pub expected_error: String,
}

impl SpecTest for SsvDutyExecutionTest {
    fn name(&self) -> &str {
        &self.name
    }

    fn setup(&mut self) {
        // No-op for parsing validation
    }

    fn run(&self) -> bool {
        println!(
            "Duty execution test '{}' parsed successfully with {} sub-tests",
            self.name,
            self.tests.len()
        );
        true
    }

    fn test_type() -> SpecTestType
    where
        Self: Sized,
    {
        SpecTestType::Ssv(SsvSpecTestType::DutyExecution)
    }
}
