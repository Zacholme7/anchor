use std::time::Duration;

use qbft::InstanceHeight;
use serde::Deserialize;
use tokio::runtime::Builder;
use types::Hash256;

use super::adapters::{
    manager::QbftManagerController,
    spec_types::{ExpectedTimerState, SpecTestCommitteeMember, TestSignedSSVMessage},
};
use crate::{
    QbftSpecTestType, SpecTest, SpecTestType,
    utils::deserializers::{
        deserialize_base64, deserialize_base64_option, deserialize_hex_hash256_option,
    },
};

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
    #[serde(rename = "Identifier", deserialize_with = "deserialize_base64")]
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

    #[serde(rename = "InputValue", deserialize_with = "deserialize_base64_option")]
    pub input_value: Option<Vec<u8>>,

    #[serde(rename = "InputMessages")]
    pub input_messages: Option<Vec<TestSignedSSVMessage>>,

    #[serde(
        rename = "ControllerPostRoot",
        deserialize_with = "deserialize_hex_hash256_option"
    )]
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

    #[serde(rename = "DecidedVal", deserialize_with = "deserialize_base64_option")]
    pub decided_value: Option<Vec<u8>>,
}

impl SpecTest for ControllerTest {
    fn name(&self) -> &str {
        &self.name
    }

    fn run(&self) -> bool {
        // Create a new runtime for each test with a unique thread name
        let test_id = format!(
            "{}_{}",
            self.name.replace(" ", "_"),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let rt = Builder::new_multi_thread()
            .enable_all()
            .worker_threads(1)
            .thread_name(test_id)
            .build()
            .unwrap();

        let result = rt.block_on(async {
            let test_controller = self.controller.as_ref().unwrap();
            let committee_member = test_controller.committee_member.clone();
            let identifier = test_controller.identifier.clone();
            let mut controller = QbftManagerController::new(committee_member, identifier, self.name.clone());
            let mut last_error: Option<String> = None;

            for (i, run_data) in self.run_instance_data.iter().enumerate() {
                // Determine the height for this RunInstanceData
                let height = run_data
                    .height
                    .map(|h| InstanceHeight::from(h as usize))
                    .unwrap_or_else(|| InstanceHeight::from(i));

                // Always try to start an instance if we have an InputValue (matching Go behavior)
                let value = run_data.input_value.clone().unwrap_or_default();
                if let Err(e) = controller.start_new_instance(height, value).await {
                    last_error = Some(e);
                }

                let mut decided_count = 0;
                let empty_messages = vec![];

                // Go through all of the run data messages
                let messages = run_data.input_messages.as_ref().unwrap_or(&empty_messages);
                for (_idx, msg) in messages.iter().enumerate() {
                    match controller.process_msg(msg).await {
                        Ok(Some(decided_data)) => {
                            decided_count += 1;
                            if let Some(expected) = &run_data.expected_decided_state {
                                if let Some(expected_bytes) = &expected.decided_value {
                                    if decided_data != *expected_bytes {
                                        last_error = Some(format!("Decided value mismatch: got {} bytes, expected {} bytes",
                                                                decided_data.len(), expected_bytes.len()));
                                    }
                                }
                            }
                        }
                        Ok(None) => {}
                        Err(e) => {
                            last_error = Some(e);
                        }
                    }
                }

                if let Some(expected) = &run_data.expected_decided_state {
                    if expected.decided_count != decided_count as u64 {
                        return false;
                    }
                }

                if let Ok(_root) = controller.get_root() {
                    // TODO: Compare with run_data.controller_post_root
                }
            }

            // The controller will be dropped and cleanup will happen via Drop trait
            drop(controller);

            if !self.expected_error.is_empty() {
                if !last_error.is_some() {
                    return false;
                }
            } else {
                if last_error.is_some() {
                    return false;
                }
            }
            true
        });

        // Properly shutdown the runtime to ensure all tasks are cleaned up
        rt.shutdown_timeout(Duration::from_millis(1000));

        // Add a small delay to ensure everything is fully cleaned up before the next test
        std::thread::sleep(Duration::from_millis(100));

        result
    }

    fn test_type() -> SpecTestType {
        SpecTestType::Qbft(QbftSpecTestType::Controller)
    }
}
