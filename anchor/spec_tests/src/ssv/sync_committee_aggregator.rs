use serde::{Deserialize, Serialize};
use ssv_types::message::SignedSSVMessage;
use std::collections::HashMap;

use crate::{SpecTest, SpecTestType, ssv::SsvSpecTestType};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SsvSyncCommitteeAggregatorTest {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Type")]
    pub test_type: String,
    #[serde(rename = "Documentation")]
    pub documentation: String,
    #[serde(rename = "Messages")]
    pub messages: Vec<SignedSSVMessage>,
    #[serde(rename = "PostDutyRunnerStateRoot")]
    pub post_duty_runner_state_root: String,
    #[serde(rename = "PostDutyRunnerState")]
    pub post_duty_runner_state: Option<String>,
    #[serde(rename = "ProofRootsMap")]
    pub proof_roots_map: HashMap<String, bool>,
    #[serde(rename = "ExpectedError")]
    pub expected_error: String,
    #[serde(rename = "OperatorID")]
    pub operator_id: u64,
}

impl SpecTest for SsvSyncCommitteeAggregatorTest {
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
        SpecTestType::Ssv(SsvSpecTestType::SyncCommitteeAggregator)
    }
}
