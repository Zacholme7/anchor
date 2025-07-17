use crate::{SpecTest, SpecTestType, SsvSpecTestType};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ssv_types::OperatorId;
use std::collections::HashMap;

// Test-specific SignedSSVMessage that doesn't use custom deserializers
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TestSignedSSVMessage {
    #[serde(rename = "Signatures")]
    pub signatures: Vec<String>,
    #[serde(rename = "OperatorIDs")]
    pub operator_ids: Vec<OperatorId>,
    #[serde(rename = "SSVMessage")]
    pub ssv_message: Value,
    #[serde(rename = "FullData")]
    pub full_data: String,
}

// Custom enum to handle mixed input types in committee tests
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum CommitteeInput {
    ValidatorDuty {
        #[serde(rename = "Slot")]
        slot: String,
        #[serde(rename = "ValidatorDuties")]
        validator_duties: Vec<ValidatorDuty>,
    },
    SignedMessage(TestSignedSSVMessage),
    // Fallback to catch anything we don't handle
    Unknown(Value),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ValidatorDuty {
    #[serde(rename = "Type")]
    pub duty_type: u64,
    #[serde(rename = "PubKey")]
    pub pub_key: String,
    #[serde(rename = "Slot")]
    pub slot: String,
    #[serde(rename = "ValidatorIndex")]
    pub validator_index: String,
    #[serde(rename = "CommitteeIndex")]
    pub committee_index: u64,
    #[serde(rename = "CommitteeLength")]
    pub committee_length: u64,
    #[serde(rename = "CommitteesAtSlot")]
    pub committees_at_slot: u64,
    #[serde(rename = "ValidatorCommitteeIndex")]
    pub validator_committee_index: u64,
    #[serde(rename = "ValidatorSyncCommitteeIndices")]
    pub validator_sync_committee_indices: Vec<u64>,
}

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
    pub input: Option<Vec<Value>>,
    #[serde(rename = "PostDutyCommitteeRoot")]
    pub post_duty_committee_root: Option<String>,
    #[serde(rename = "OutputMessages")]
    pub output_messages: Option<Vec<Value>>,
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
    pub input: Vec<Value>,
    #[serde(rename = "PostDutyCommitteeRoot")]
    pub post_duty_committee_root: String,
    #[serde(rename = "OutputMessages")]
    pub output_messages: Vec<Value>,
    #[serde(rename = "BeaconBroadcastedRoots")]
    pub beacon_broadcasted_roots: Option<Vec<String>>,
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
            for _test in tests {
                // todo!()
            }
        } else {
            // todo!()
        }
        true
    }

    fn test_type() -> SpecTestType {
        SpecTestType::Ssv(SsvSpecTestType::Committee)
    }
}
