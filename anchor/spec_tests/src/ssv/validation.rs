use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{SpecTest, SpecTestType, SsvSpecTestType};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ValidationSubTest {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Type")]
    pub test_type: String,
    #[serde(rename = "Documentation")]
    pub documentation: String,
    #[serde(rename = "Network")]
    pub network: String,
    #[serde(rename = "RunnerRole")]
    pub runner_role: u64,
    #[serde(rename = "DutySlot")]
    pub duty_slot: String,
    #[serde(rename = "Input")]
    pub input: String,
    #[serde(rename = "SlashableSlots")]
    pub slashable_slots: Option<Value>,
    #[serde(rename = "omitempty")]
    pub omitempty: Option<Value>,
    #[serde(rename = "ExpectedError")]
    pub expected_error: String,
    #[serde(rename = "AnyError")]
    pub any_error: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SsvValidationTest {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Type")]
    pub test_type: String,
    #[serde(rename = "Documentation")]
    pub documentation: String,

    // Fields for single test format (SpecTest_*)
    #[serde(rename = "Network")]
    pub network: Option<String>,
    #[serde(rename = "RunnerRole")]
    pub runner_role: Option<u64>,
    #[serde(rename = "DutySlot")]
    pub duty_slot: Option<String>,
    #[serde(rename = "Input")]
    pub input: Option<String>,
    #[serde(rename = "SlashableSlots")]
    pub slashable_slots: Option<Value>,
    #[serde(rename = "omitempty")]
    pub omitempty: Option<Value>,
    #[serde(rename = "ExpectedError")]
    pub expected_error: Option<String>,
    #[serde(rename = "AnyError")]
    pub any_error: Option<bool>,

    // Field for multi test format (MultiSpecTest_*)
    #[serde(rename = "Tests")]
    pub tests: Option<Vec<ValidationSubTest>>,
}

impl SpecTest for SsvValidationTest {
    fn name(&self) -> &str {
        &self.name
    }

    fn setup(&mut self) {
        // No-op for parsing validation
    }

    fn run(&self) -> bool {
        true
    }

    fn test_type() -> SpecTestType {
        SpecTestType::Ssv(SsvSpecTestType::Validation)
    }
}
