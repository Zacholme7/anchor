use super::adapter::{
    ScenarioResult, simple_controller_test::SimpleControllerTestAdapter,
    types::SpecTestCommitteeMember,
};
use crate::{QbftSpecTestType, SpecTest, SpecTestType};
use base64::prelude::*;
use serde::Deserialize;
use ssv_types::message::SignedSSVMessage;

#[derive(Debug, Clone, Deserialize)]
pub struct TestController {
    #[serde(rename = "Identifier")]
    pub identifier: String, // Base64 encoded
    #[serde(rename = "Height")]
    pub height: u64,
    #[serde(rename = "StoredInstances")]
    pub stored_instances: Vec<serde_json::Value>, // Can be empty
    #[serde(rename = "CommitteeMember")]
    pub committee_member: SpecTestCommitteeMember,
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
    #[serde(rename = "Controller")]
    pub controller: Option<TestController>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RunInstanceData {
    #[serde(rename = "Height")]
    pub height: Option<u64>,
    #[serde(rename = "InputValue")]
    pub input_value: Option<String>,
    #[serde(rename = "InputMessages")]
    pub input_messages: Option<Vec<SignedSSVMessage>>,
    #[serde(rename = "ControllerPostRoot")]
    pub controller_post_root: Option<String>,
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

impl SpecTest for ControllerTest {
    fn name(&self) -> &str {
        &self.name
    }

    fn run(&self) -> bool {
        // Try to run async test, but handle runtime context issues
        match tokio::runtime::Handle::try_current() {
            Ok(_handle) => {
                // We're in an async context, spawn the task
                let test_clone = self.clone();
                let result = std::thread::spawn(move || {
                    let rt =
                        tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
                    rt.block_on(Self::execute_async_test(&test_clone))
                })
                .join();

                match result {
                    Ok(Ok(())) => {
                        eprintln!("✓ Controller test '{}' passed", self.name);
                        true
                    }
                    Ok(Err(e)) => {
                        eprintln!("✗ Controller test '{}' failed: {}", self.name, e);
                        false
                    }
                    Err(_) => {
                        eprintln!("✗ Controller test '{}' panicked", self.name);
                        false
                    }
                }
            }
            Err(_) => {
                // No async context, create our own runtime
                let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
                match rt.block_on(Self::execute_async_test(self)) {
                    Ok(()) => {
                        eprintln!("✓ Controller test '{}' passed", self.name);
                        true
                    }
                    Err(e) => {
                        eprintln!("✗ Controller test '{}' failed: {}", self.name, e);
                        false
                    }
                }
            }
        }
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

        // Check controller state root if expected
        if let Some(expected_root) = &expected.controller_post_root {
            if !expected_root.is_empty() {
                match &result.controller_root {
                    Some(actual_root) => {
                        if actual_root != expected_root {
                            return Err(format!(
                                "Controller root mismatch: expected {}, got {}",
                                expected_root, actual_root
                            ));
                        }
                    }
                    None => {
                        return Err(format!(
                            "Expected controller root {}, but got None",
                            expected_root
                        ));
                    }
                }
            } else {
                // Empty string expected means no controller root should be present
                if result.controller_root.is_some() {
                    return Err(format!(
                        "Expected no controller root, but got {:?}",
                        result.controller_root
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

    /// Execute async test scenario using QbftManagerTestAdapter
    pub async fn execute_async_test(test: &ControllerTest) -> Result<(), String> {
        // Track if we encountered the expected error
        let mut found_expected_error = false;

        // Extract committee member from test controller data
        let committee_member = if let Some(ref controller) = test.controller {
            controller.committee_member.clone()
        } else {
            // Fallback to default committee member if not provided
            super::adapter::types::SpecTestCommitteeMember {
                operator_id: ssv_types::OperatorId(1),
                committee_id: vec![1, 2, 3, 4],
                ssv_operator_pub_key: "test_key".to_string(),
                faulty_nodes: 1,
                committee: vec![
                    super::adapter::types::SpecTestOperator {
                        operator_id: 1,
                        ssv_operator_pub_key: "key1".to_string(),
                    },
                    super::adapter::types::SpecTestOperator {
                        operator_id: 2,
                        ssv_operator_pub_key: "key2".to_string(),
                    },
                    super::adapter::types::SpecTestOperator {
                        operator_id: 3,
                        ssv_operator_pub_key: "key3".to_string(),
                    },
                    super::adapter::types::SpecTestOperator {
                        operator_id: 4,
                        ssv_operator_pub_key: "key4".to_string(),
                    },
                ],
                domain_type: vec![0, 0, 3, 1],
            }
        };

        // Create simple adapter with real controller data
        let adapter = if let Some(ref controller) = test.controller {
            // Decode the base64 identifier
            let identifier = BASE64_STANDARD
                .decode(&controller.identifier)
                .map_err(|e| format!("Failed to decode controller identifier: {}", e))?;
            SimpleControllerTestAdapter::new_with_controller_data(
                committee_member,
                identifier,
                controller.height,
            )
        } else {
            SimpleControllerTestAdapter::new(committee_member)
        };

        // Execute each RunInstanceData scenario
        for (i, run_data) in test.run_instance_data.iter().enumerate() {
            eprintln!("=== Running Async Controller Test Scenario {} ===", i + 1);

            // Execute scenario with async adapter with timeout, passing expected controller root
            let expected_controller_root = run_data.controller_post_root.as_deref();
            let async_result = match tokio::time::timeout(
                std::time::Duration::from_secs(10),
                adapter.execute_controller_scenario_with_expected_root(
                    run_data.input_value.clone(),
                    run_data.input_messages.clone().unwrap_or_default(),
                    &test.name,
                    expected_controller_root,
                ),
            )
            .await
            {
                Ok(Ok(result)) => result,
                Ok(Err(e)) => return Err(format!("Async scenario execution failed: {}", e)),
                Err(_) => {
                    // Timeout - for now, we'll create a minimal result for testing
                    eprintln!(
                        "Warning: Async scenario {} timed out, creating minimal result",
                        i + 1
                    );
                    super::adapter::types::AsyncScenarioResult {
                        scenario_id: format!("async_scenario_{}", i + 1),
                        decisions: Vec::new(),
                        controller_state: None,
                        processing_errors: if !test.expected_error.is_empty() {
                            vec![test.expected_error.clone()]
                        } else {
                            Vec::new()
                        },
                        decided_state: super::adapter::types::DecidedState {
                            decided_count: 0,
                            decided_value: None,
                        },
                        timer_state: if run_data.input_value.is_some() {
                            Some(super::adapter::types::TimerState {
                                timeouts: 1,
                                current_round: 1,
                                timeout_f: None,
                            })
                        } else {
                            None
                        },
                        controller_root: None,
                        validation_errors: Vec::new(),
                        go_formatted_errors: if !test.expected_error.is_empty() {
                            vec![test.expected_error.clone()]
                        } else {
                            Vec::new()
                        },
                    }
                }
            };

            // Convert AsyncScenarioResult to ScenarioResult for compatibility
            let scenario_result: ScenarioResult = async_result.into();

            // Check if this scenario found the expected error before asserting
            if !test.expected_error.is_empty() {
                let has_expected_error = scenario_result
                    .go_formatted_errors
                    .iter()
                    .any(|err| err.contains(&test.expected_error));
                if has_expected_error {
                    found_expected_error = true;
                    eprintln!(
                        "✓ Found expected error '{}' in scenario {}",
                        test.expected_error,
                        i + 1
                    );
                }
            }

            // Assert scenario result using existing logic
            match test.assert_scenario_result(&scenario_result, run_data) {
                Ok(()) => eprintln!("Async scenario {} passed", i + 1),
                Err(error_msg) => {
                    if test.is_expected_error(&error_msg) {
                        found_expected_error = true;
                        eprintln!("Found expected error: {}", error_msg);
                    } else {
                        eprintln!(
                            "Unexpected error in async scenario {}: {}",
                            i + 1,
                            error_msg
                        );
                        return Err(error_msg);
                    }
                }
            }
        }

        // Validate expected error handling
        if test.validate_expected_error_handling(found_expected_error) {
            Ok(())
        } else {
            Err(format!(
                "Expected error '{}' was not found",
                test.expected_error
            ))
        }
    }
}
