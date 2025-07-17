use serde::{Deserialize, Serialize};
use serde_json::Value;
use ssv_types::message::SignedSSVMessage;

use crate::{SpecTest, SpecTestType, ssv::SsvSpecTestType};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NewDutySubTest {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Runner")]
    pub runner: Value, // Using Value for complex nested structure
    #[serde(rename = "Duty")]
    pub duty: Value, // Using Value for complex duty structure
    #[serde(rename = "PostDutyRunnerStateRoot")]
    pub post_duty_runner_state_root: String,
    #[serde(rename = "PostDutyRunnerState")]
    pub post_duty_runner_state: Option<String>,
    #[serde(rename = "OutputMessages")]
    pub output_messages: Vec<SignedSSVMessage>,
    #[serde(rename = "BeaconBroadcastedRoots")]
    pub beacon_broadcasted_roots: Vec<String>,
    #[serde(rename = "ExpectedError")]
    pub expected_error: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SsvNewDutyTest {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Type")]
    pub test_type: String,
    #[serde(rename = "Documentation")]
    pub documentation: String,
    #[serde(rename = "Tests")]
    pub tests: Vec<NewDutySubTest>,
}

impl SpecTest for SsvNewDutyTest {
    fn name(&self) -> &str {
        &self.name
    }

    fn setup(&mut self) {
        // No-op for now
    }

    fn run(&self) -> bool {
        true
    }

    fn test_type() -> SpecTestType {
        SpecTestType::Ssv(SsvSpecTestType::NewDuty)
    }
}
