use serde::{Deserialize, Serialize};
use serde_json::Value;
use ssv_types::message::SignedSSVMessage;

use crate::{SpecTest, SpecTestType, SsvSpecTestType};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SsvControllerTest {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Type")]
    pub test_type: String,
    #[serde(rename = "Documentation")]
    pub documentation: String,
    #[serde(rename = "RunInstanceData")]
    pub run_instance_data: Vec<ControllerRunInstance>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ControllerRunInstance {
    #[serde(rename = "InputValue")]
    pub input_value: Option<String>,
    #[serde(rename = "InputMessages")]
    pub input_messages: Vec<SignedSSVMessage>,
    #[serde(rename = "ControllerPostRoot")]
    pub controller_post_root: String,
    #[serde(rename = "ControllerPostState")]
    pub controller_post_state: Option<Value>,
    #[serde(rename = "ExpectedError")]
    pub expected_error: String,
}

impl SpecTest for SsvControllerTest {
    fn name(&self) -> &str {
        &self.name
    }

    fn setup(&mut self) {
        // No-op for now - parsing validation only
    }

    fn run(&self) -> bool {
        // No-op implementation - always return true for parsing validation
        println!("Controller test '{}' parsed successfully", self.name);
        true
    }

    fn test_type() -> SpecTestType
    where
        Self: Sized,
    {
        SpecTestType::Ssv(SsvSpecTestType::Controller)
    }
}
