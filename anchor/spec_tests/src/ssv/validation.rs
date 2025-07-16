use serde::{Deserialize, Serialize};
use serde_json::Value;
use types::Slot;

use crate::{SpecTest, SpecTestType, SsvSpecTestType};
use crate::types::{ValidationContext, SlashableSlots};
use crate::utils::ValidationEngine;

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
        if let Some(ref tests) = self.tests {
            // Multi test format - execute all sub-tests
            let mut all_passed = true;
            for (i, test) in tests.iter().enumerate() {
                let passed = self.run_validation_subtest(test);
                println!("  Sub-test {} '{}': {}", i + 1, test.name, if passed { "PASS" } else { "FAIL" });
                if !passed {
                    all_passed = false;
                }
            }
            println!("Validation multi-test '{}': {} ({}/{} sub-tests passed)", 
                self.name, 
                if all_passed { "PASS" } else { "FAIL" },
                tests.iter().filter(|t| self.run_validation_subtest(t)).count(),
                tests.len()
            );
            all_passed
        } else {
            // Single test format
            let passed = self.run_single_validation();
            println!("Validation test '{}': {}", self.name, if passed { "PASS" } else { "FAIL" });
            passed
        }
    }

    fn test_type() -> SpecTestType
    where
        Self: Sized,
    {
        SpecTestType::Ssv(SsvSpecTestType::Validation)
    }
}

impl SsvValidationTest {
    fn run_single_validation(&self) -> bool {
        // Extract single test data
        let runner_role = self.runner_role.unwrap_or(0);
        let duty_slot = Slot::new(self.duty_slot.as_ref().unwrap().parse().unwrap_or(0));
        let input = self.input.as_ref().unwrap();
        let expected_error = self.expected_error.as_ref();
        let slashable_slots = self.slashable_slots.as_ref()
            .and_then(|v| serde_json::from_value::<SlashableSlots>(v.clone()).ok())
            .unwrap_or_default();

        // Create validation context and engine
        let context = ValidationContext::new(slashable_slots);
        let engine = ValidationEngine::new(context);

        // Execute validation
        let result = engine.validate_by_role(runner_role, input, duty_slot);

        // Check result against expected
        match (&result, expected_error) {
            (Err(actual_error), Some(expected)) => {
                let actual_str = actual_error.to_string();
                let matches = actual_str == *expected || expected.is_empty();
                if !matches {
                    println!("    Expected error: '{}', got: '{}'", expected, actual_str);
                }
                matches
            },
            (Ok(()), None) => true,
            (Ok(()), Some(ref e)) if e.is_empty() => true,
            (Ok(()), Some(expected)) => {
                println!("    Expected error: '{}', got: success", expected);
                false
            },
            (Err(actual_error), None) => {
                println!("    Expected success, got error: '{}'", actual_error);
                false
            }
        }
    }
    
    fn run_validation_subtest(&self, test: &ValidationSubTest) -> bool {
        let duty_slot = Slot::new(test.duty_slot.parse().unwrap_or(0));
        let slashable_slots = test.slashable_slots.as_ref()
            .and_then(|v| serde_json::from_value::<SlashableSlots>(v.clone()).ok())
            .unwrap_or_default();

        let context = ValidationContext::new(slashable_slots);
        let engine = ValidationEngine::new(context);

        let result = engine.validate_by_role(test.runner_role, &test.input, duty_slot);

        match (&result, &test.expected_error) {
            (Err(actual_error), expected) => {
                let actual_str = actual_error.to_string();
                let matches = actual_str == *expected || expected.is_empty();
                if !matches {
                    println!("    Expected error: '{}', got: '{}' (role {})", expected, actual_str, test.runner_role);
                }
                matches
            },
            (Ok(()), expected) => {
                let matches = expected.is_empty();
                if !matches {
                    println!("    Expected error: '{}', got: success (role {})", expected, test.runner_role);
                }
                matches
            },
        }
    }
}
