use serde::Deserialize;
use ssv_types::OperatorId;
use ssv_types::message::SignedSSVMessage;

use super::adapter::{QbftTestAdapter, ScenarioResult, TestContext, TestType};
use crate::{QbftSpecTestType, SpecTest, SpecTestType};

impl SpecTest for ControllerTest {
    fn name(&self) -> &str {
        &self.name
    }

    fn run(&self) -> bool {
        // Track if we encountered the expected error
        let mut found_expected_error = false;

        // Execute each RunInstanceData scenario
        for (i, run_data) in self.run_instance_data.iter().enumerate() {
            eprintln!("=== Running Controller Test Scenario {} ===", i + 1);

            // Create test context
            let test_context = TestContext::new(
                format!("{}_scenario_{}", self.name, i + 1),
                TestType::Controller,
            )
            .with_expected_errors(vec![self.expected_error.clone()]);

            // Determine height for this scenario
            let scenario_height = run_data.height.unwrap_or(i as u64);

            // Create adapter for scenario
            let mut adapter = match QbftTestAdapter::with_default_committee() {
                Ok(adapter) => adapter.with_test_context(test_context),
                Err(e) => {
                    eprintln!("Failed to create adapter: {}", e);
                    return false;
                }
            };

            // Execute scenario using unified adapter
            let scenario_result = adapter.execute_controller_scenario(
                run_data.input_value.clone(),
                run_data.input_messages.clone().unwrap_or_default(),
            );

            // Assert scenario result
            match self.assert_scenario_result(&scenario_result, run_data) {
                Ok(()) => eprintln!("Scenario {} passed", i + 1),
                Err(error_msg) => {
                    if self.is_expected_error(&error_msg) {
                        found_expected_error = true;
                        eprintln!("Found expected error: {}", error_msg);
                    } else {
                        eprintln!("Unexpected error in scenario {}: {}", i + 1, error_msg);
                        return false;
                    }
                }
            }
        }

        self.validate_expected_error_handling(found_expected_error)
    }

    fn setup(&mut self) {
        // No setup needed for controller tests
    }

    fn test_type() -> SpecTestType {
        SpecTestType::Qbft(QbftSpecTestType::Controller)
    }
}

impl ControllerTest {
    /// Assert scenario result matches expectations
    fn assert_scenario_result(
        &self,
        result: &ScenarioResult,
        expected: &RunInstanceData,
    ) -> Result<(), String> {
        // Check decided state if expected
        if let Some(expected_decided) = &expected.expected_decided_state {
            let actual_decided = &result.decided_state;
            if actual_decided.decided_count != expected_decided.decided_count {
                return Err(format!(
                    "Decided count mismatch: expected {}, got {}",
                    expected_decided.decided_count, actual_decided.decided_count
                ));
            }
        }

        // Check timer state if expected
        if let Some(expected_timer) = &expected.expected_timer_state {
            if let Some(actual_timer) = &result.timer_state {
                if actual_timer.timeouts != expected_timer.timeouts {
                    return Err(format!(
                        "Timer timeouts mismatch: expected {}, got {}",
                        expected_timer.timeouts, actual_timer.timeouts
                    ));
                }
            }
        }

        // Check for expected errors in Go format
        if !self.expected_error.is_empty() {
            if !result
                .go_formatted_errors
                .iter()
                .any(|err| err.contains(&self.expected_error))
            {
                return Err(format!(
                    "Expected error '{}' not found in: {:?}",
                    self.expected_error, result.go_formatted_errors
                ));
            }
        }

        Ok(())
    }

    /// Check if error message matches expected error
    fn is_expected_error(&self, error: &str) -> bool {
        !self.expected_error.is_empty() && error.contains(&self.expected_error)
    }

    /// Validate that expected error handling worked correctly
    fn validate_expected_error_handling(&self, found_expected_error: bool) -> bool {
        if self.expected_error.is_empty() {
            // No error expected, test should have passed
            true
        } else {
            // Error expected, should have been found
            if found_expected_error {
                eprintln!(
                    "✓ Expected error '{}' was correctly found",
                    self.expected_error
                );
                true
            } else {
                eprintln!("✗ Expected error '{}' was not found", self.expected_error);
                false
            }
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ControllerTest {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Type")]
    pub test_type: String,
    #[serde(rename = "Documentation")]
    pub documentation: String,
    #[serde(rename = "RunInstanceData")]
    pub run_instance_data: Vec<RunInstanceData>,
    #[serde(rename = "ExpectedError")]
    pub expected_error: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RunInstanceData {
    #[serde(rename = "Height")]
    pub height: Option<u64>,
    #[serde(rename = "InputValue")]
    pub input_value: Option<String>,
    #[serde(rename = "InputMessages")]
    pub input_messages: Option<Vec<SignedSSVMessage>>,
    #[serde(rename = "ExpectedDecidedState")]
    pub expected_decided_state: Option<ExpectedDecidedState>,
    #[serde(rename = "ExpectedTimerState")]
    pub expected_timer_state: Option<ExpectedTimerState>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExpectedDecidedState {
    #[serde(rename = "DecidedCnt")]
    pub decided_count: u64,
    #[serde(rename = "DecidedVal")]
    pub decided_value: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExpectedTimerState {
    #[serde(rename = "Timeouts")]
    pub timeouts: u64,
    #[serde(rename = "Round")]
    pub round: Option<u64>,
}
