use serde::{Deserialize, Serialize};
use ssv_types::partial_sig::PartialSignatureMessage;

use crate::{SpecTest, SpecTestType, SsvSpecTestType};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SsvPartialSignatureTest {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Type")]
    pub test_type: String,
    #[serde(rename = "Documentation")]
    pub documentation: String,
    #[serde(rename = "Quorum")]
    pub quorum: u64,
    #[serde(rename = "ValidatorPubKey")]
    pub validator_pub_key: String,
    #[serde(rename = "SignatureMsgs")]
    pub signature_msgs: Vec<PartialSignatureMessage>,
    #[serde(rename = "ExpectedError")]
    pub expected_error: String,
    #[serde(rename = "ExpectedResult")]
    pub expected_result: Option<String>,
    #[serde(rename = "ExpectedQuorum")]
    pub expected_quorum: bool,
}

impl SpecTest for SsvPartialSignatureTest {
    fn name(&self) -> &str {
        &self.name
    }

    fn setup(&mut self) {
        // No-op for parsing validation
    }

    fn run(&self) -> bool {
        println!("Partial signature test '{}' parsed successfully", self.name);
        true
    }

    fn test_type() -> SpecTestType
    where
        Self: Sized,
    {
        SpecTestType::Ssv(SsvSpecTestType::PartialSignatures)
    }
}
