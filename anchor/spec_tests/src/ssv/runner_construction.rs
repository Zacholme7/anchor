use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use crate::{SpecTest, SpecTestType, ssv::SsvSpecTestType};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CommitteeMember {
    #[serde(rename = "SharePubKey")]
    pub share_pub_key: String,
    #[serde(rename = "Signer")]
    pub signer: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TestShare {
    #[serde(rename = "ValidatorIndex")]
    pub validator_index: String,
    #[serde(rename = "ValidatorPubKey")]
    pub validator_pub_key: Vec<u8>,
    #[serde(rename = "SharePubKey")]
    pub share_pub_key: String,
    #[serde(rename = "Committee")]
    pub committee: Vec<CommitteeMember>,
    #[serde(rename = "DomainType")]
    pub domain_type: Vec<u8>,
    #[serde(rename = "FeeRecipientAddress")]
    pub fee_recipient_address: Vec<u8>,
    #[serde(rename = "Graffiti")]
    pub graffiti: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SsvRunnerConstructionTest {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Type")]
    pub test_type: String,
    #[serde(rename = "Shares")]
    pub shares: HashMap<String, TestShare>,
    #[serde(rename = "RoleError")]
    pub role_error: HashMap<String, String>,
}

impl SpecTest for SsvRunnerConstructionTest {
    fn name(&self) -> &str {
        &self.name
    }

    fn setup(&mut self) {
        // No-op for now
    }

    fn run(&self) -> bool {
        println!(
            "Runner construction test '{}' parsed successfully with {} shares",
            self.name,
            self.shares.len()
        );
        for (key, share) in &self.shares {
            println!(
                "  Share {}: validator_index={}, committee_size={}",
                key,
                share.validator_index,
                share.committee.len()
            );
        }
        println!("  Role errors: {:?}", self.role_error);
        true
    }

    fn test_type() -> SpecTestType {
        SpecTestType::Ssv(SsvSpecTestType::RunnerConstruction)
    }
}
