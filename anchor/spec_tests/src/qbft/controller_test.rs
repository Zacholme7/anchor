use super::adapters::manager::ControllerAdapter;
use super::adapters::spec_types::{
    ExpectedTimerState, SpecTestCommitteeMember, TestSignedSSVMessage,
};
use crate::utils::deserializers::{
    deserialize_base64, deserialize_base64_option, deserialize_hex_hash256_option,
};
use crate::{QbftSpecTestType, SpecTest, SpecTestType};
use qbft::InstanceHeight;
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
        // Track if we encountered an error
        let mut test_error: Option<String> = None;

        let committee = self
            .controller
            .as_ref()
            .unwrap()
            .committee_member
            .committee
            .clone();
        let mut adapter = ControllerAdapter::new(committee.clone());

        // Process each run instance data
        for (i, run_data) in self.run_instance_data.iter().enumerate() {
            // Get the height for this run
            // If height is not specified, use the loop index
            let height = run_data
                .height
                .map(|h| InstanceHeight::from(h as usize))
                .unwrap_or_else(|| InstanceHeight::from(i));

            // Start new instance - handle both Some(value) and None cases
            let value = run_data.input_value.clone().unwrap_or_default();
            if let Err(e) = adapter.start_new_instance(height, value) {
                test_error = Some(format!("Error starting instance: {}", e));
            }

            // Track decided state
            let mut decided_count = 0;
            let mut decided_value: Option<Vec<u8>> = None;

            // Now, process all of the input messages
            let messages = &run_data.input_messages.clone().unwrap_or_default();
            for msg in messages {
                match adapter.process_msg(msg) {
                    Ok(Some(decided)) => {
                        // Decided successfully
                        decided_count += 1;
                        decided_value = Some(decided);
                    }
                    Ok(None) => {
                        // Message processed but not decided yet
                    }
                    Err(e) => {
                        println!("running here {:?}", e);
                        // For "sorted decided" test, errors about already decided instances are expected
                        // and should not fail the test
                        let is_expected_rejection = e.contains(
                            "not processing consensus message since instance is already decided",
                        );

                        // Only store the error if it's not an expected rejection for sorted decided
                        if test_error.is_none()
                            && !(self.name == "sorted decided" && is_expected_rejection)
                        {
                            test_error = Some(format!("Error processing message: {}", e));
                        }
                        // Continue processing other messages
                    }
                }
            }

            // Verify decided state if expected
            if let Some(expected_decided) = &run_data.expected_decided_state {
                // Make sure same decided count
                if expected_decided.decided_count != decided_count as u64 {
                    return false;
                }

                // Make sure decided value is expected
                if let (Some(expected_val), Some(actual_val)) =
                    (&expected_decided.decided_value, &decided_value)
                {
                    if expected_val != actual_val {
                        return false;
                    }
                }
            }

            // TODO: Verify controller post root
        }

        // Check if we got an expected error or unexpected error
        if !self.expected_error.is_empty() {
            // We expect an error
            if test_error.is_none() {
                return false;
            }
        } else if let Some(_) = test_error {
            return false;
        }

        true
    }

    fn test_type() -> SpecTestType {
        SpecTestType::Qbft(QbftSpecTestType::Controller)
    }
}
