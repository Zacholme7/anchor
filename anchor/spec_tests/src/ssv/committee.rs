use crate::{SpecTest, SpecTestType, SsvSpecTestType};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ssv_types::{OperatorId, message::SignedSSVMessage};
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SsvCommitteeTest {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Type")]
    pub test_type: Option<String>,
    #[serde(rename = "Documentation")]
    pub documentation: Option<String>,

    // Fields for single test format (CommitteeSpecTest_*)
    #[serde(rename = "Committee")]
    pub committee: Option<Committee>,
    #[serde(rename = "Input")]
    pub input: Option<Vec<SignedSSVMessage>>,
    #[serde(rename = "PostDutyCommitteeRoot")]
    pub post_duty_committee_root: Option<String>,
    #[serde(rename = "OutputMessages")]
    pub output_messages: Option<Vec<SignedSSVMessage>>,
    #[serde(rename = "BeaconBroadcastedRoots")]
    pub beacon_broadcasted_roots: Option<Vec<String>>,
    #[serde(rename = "ExpectedError")]
    pub expected_error: Option<String>,
    #[serde(rename = "omitempty")]
    pub omitempty: Option<String>,

    // Field for multi test format (MultiCommitteeSpecTest_*)
    #[serde(rename = "Tests")]
    pub tests: Option<Vec<CommitteeSubTest>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CommitteeSubTest {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "omitempty")]
    pub omitempty: Option<String>,
    #[serde(rename = "Documentation")]
    pub documentation: Option<String>,
    #[serde(rename = "Committee")]
    pub committee: Committee,
    #[serde(rename = "Input")]
    pub input: Vec<SignedSSVMessage>,
    #[serde(rename = "PostDutyCommitteeRoot")]
    pub post_duty_committee_root: String,
    #[serde(rename = "OutputMessages")]
    pub output_messages: Vec<SignedSSVMessage>,
    #[serde(rename = "BeaconBroadcastedRoots")]
    pub beacon_broadcasted_roots: Vec<String>,
    #[serde(rename = "ExpectedError")]
    pub expected_error: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Committee {
    #[serde(rename = "Runners")]
    pub runners: HashMap<String, Value>,
    #[serde(rename = "CommitteeMember")]
    pub committee_member: CommitteeMember,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CommitteeMember {
    #[serde(rename = "OperatorID")]
    pub operator_id: OperatorId,
    #[serde(rename = "CommitteeID")]
    pub committee_id: Vec<u8>,
    #[serde(rename = "SSVOperatorPubKey")]
    pub ssv_operator_pub_key: String,
    #[serde(rename = "FaultyNodes")]
    pub faulty_nodes: u64,
    #[serde(rename = "Committee")]
    pub committee: Vec<CommitteeOperator>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CommitteeOperator {
    #[serde(rename = "OperatorID")]
    pub operator_id: OperatorId,
    #[serde(rename = "SSVOperatorPubKey")]
    pub ssv_operator_pub_key: String,
}

impl SpecTest for SsvCommitteeTest {
    fn name(&self) -> &str {
        &self.name
    }

    fn setup(&mut self) {
        // No-op for parsing validation
    }

    fn run(&self) -> bool {
        if let Some(ref tests) = self.tests {
            // Multi test format (MultiCommitteeSpecTest_*)
            println!(
                "Committee multi-test '{}' parsed successfully with {} sub-tests",
                self.name,
                tests.len()
            );
            for test in tests {
                println!("  Sub-test '{}' parsed successfully", test.name);
            }
        } else {
            // Single test format (CommitteeSpecTest_*)
            println!("Committee test '{}' parsed successfully", self.name);
        }
        true
    }

    fn test_type() -> SpecTestType
    where
        Self: Sized,
    {
        SpecTestType::Ssv(SsvSpecTestType::Committee)
    }
}
