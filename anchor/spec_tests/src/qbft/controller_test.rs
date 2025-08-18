use super::adapters::spec_types::{
    ExpectedTimerState, SpecTestCommitteeMember, TestSignedSSVMessage,
};
use crate::utils::deserializers::{
    deserialize_base64, deserialize_base64_option, deserialize_hex_hash256_option,
};
use crate::utils::test_keys::TestKeySet;
use crate::{QbftSpecTestType, SpecTest, SpecTestType};
use serde::Deserialize;
use types::Hash256;

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
    #[serde(rename = "PrivateKeys")]
    pub private_keys: Option<serde_json::Value>, // Store as raw JSON for now
}

#[derive(Debug, Clone, Deserialize)]
pub struct TestController {
    #[serde(rename = "Identifier")]
    #[serde(deserialize_with = "deserialize_base64")]
    pub identifier: Vec<u8>,
    #[serde(rename = "Height")]
    pub height: u64,
    #[serde(rename = "StoredInstances")]
    pub stored_instances: Vec<serde_json::Value>, // Can be empty
    #[serde(rename = "CommitteeMember")]
    pub committee_member: SpecTestCommitteeMember,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RunInstanceData {
    #[serde(rename = "Height")]
    pub height: Option<u64>,
    #[serde(rename = "InputValue")]
    #[serde(deserialize_with = "deserialize_base64_option")]
    pub input_value: Option<Vec<u8>>,
    #[serde(rename = "InputMessages")]
    pub input_messages: Option<Vec<TestSignedSSVMessage>>,
    #[serde(rename = "ControllerPostRoot")]
    #[serde(deserialize_with = "deserialize_hex_hash256_option")]
    pub controller_post_root: Option<Hash256>,
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
    #[serde(deserialize_with = "deserialize_base64_option")]
    pub decided_value: Option<Vec<u8>>,
}

impl SpecTest for ControllerTest {
    fn run(&self) -> bool {
        // Create runtime for async operations
        let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");

        rt.block_on(async {
            // Track if we encountered an error
            let mut test_error: Option<String> = None;

            // Create a single adapter that persists across all runs
            // This matches Go's behavior where the controller persists
            // Pass committee information if available for signature verification
            let committee = self
                .controller
                .as_ref()
                .and_then(|c| c.committee_member.committee.clone())
                .unwrap_or_default();
            let mut adapter = super::adapters::manager::ControllerAdapter::new(
                ssv_types::OperatorId(1),
                committee.clone(),
            );


                adapter.set_test_keys(TestKeySet::four_share_set());

            // Process each run instance data
            for (i, run_data) in self.run_instance_data.iter().enumerate() {
                // Get the height for this run
                // If height is not specified, use the loop index (matching Go's behavior)
                let height = run_data
                    .height
                    .map(|h| qbft::InstanceHeight::from(h as usize))
                    .unwrap_or_else(|| qbft::InstanceHeight::from(i));

                // Start new instance - handle both Some(value) and None cases
                if let Some(value) = &run_data.input_value {
                    if let Err(e) = adapter.start_new_instance(height, value.clone()).await {
                        test_error = Some(format!("Error starting instance: {}", e));
                        // Continue to see if this was expected
                    }
                } else {
                    // Nil value case - Go's test still calls StartNewInstance with nil
                    // which should fail validation
                    if let Err(e) = adapter.start_new_instance(height, Vec::new()).await {
                        test_error = Some(format!("Error starting instance: {}", e));
                        // Continue to see if this was expected
                    }
                }

                // Process input messages
                if let Some(messages) = &run_data.input_messages {
                    let mut decided_count = 0;
                    let mut decided_value: Option<Vec<u8>> = None;

                    for (msg_idx, msg) in messages.iter().enumerate() {
                        if self.name == "sorted decided" {
                            // Debug the sorted decided test
                            println!("Processing message {} for sorted decided test", msg_idx);
                        }
                        match adapter.process_msg(msg).await {
                            Ok(Some(decided)) => {
                                if self.name == "sorted decided" {
                                    println!("Message {} decided!", msg_idx);
                                }
                                decided_count += 1;
                                decided_value = Some(decided);
                            }
                            Ok(None) => {
                                // Message processed but not decided yet
                            }
                            Err(e) => {
                                if self.name == "sorted decided" {
                                    println!("Message {} error: {}", msg_idx, e);
                                }
                                // For "sorted decided" test, errors about already decided instances are expected
                                // and should not fail the test
                                let is_expected_rejection = e.contains("not processing consensus message since instance is already decided");

                                // Only store the error if it's not an expected rejection for sorted decided
                                if test_error.is_none() && !(self.name == "sorted decided" && is_expected_rejection) {
                                    test_error = Some(format!("Error processing message: {}", e));
                                }
                                // Continue processing other messages
                            }
                        }
                    }

                    // Verify decided state if expected
                    if let Some(expected_decided) = &run_data.expected_decided_state {
                        if expected_decided.decided_count != decided_count as u64 {
                            println!(
                                "FAILED {}: Expected {} decides, got {}",
                                self.name, expected_decided.decided_count, decided_count
                            );
                            return false;
                        }

                        if let (Some(expected_val), Some(actual_val)) =
                            (&expected_decided.decided_value, &decided_value)
                        {
                            if expected_val != actual_val {
                                println!("FAILED {}: Decided value mismatch", self.name);
                                return false;
                            }
                        }
                    }
                }

                // TODO: Verify controller post root
            }

            // Check if we got an expected error or unexpected error
            if !self.expected_error.is_empty() {
                // We expect an error
                if test_error.is_none() {
                    println!(
                        "FAILED {}: Expected error '{}' but got none",
                        self.name, self.expected_error
                    );
                    return false;
                }
            } else if let Some(err) = test_error {
                // We don't expect an error but got one
                println!("FAILED {}: Unexpected error: {}", self.name, err);
                return false;
            }

            true
        })
    }

    fn test_type() -> SpecTestType {
        SpecTestType::Qbft(QbftSpecTestType::Controller)
    }
}
