use super::adapters::manager::QbftManagerController;
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
    fn run(&self) -> bool {
        let rt = tokio::runtime::Runtime::new().unwrap();

        rt.block_on(async {
            let committee_member = self.controller.as_ref().unwrap().committee_member.clone();
            let mut controller = QbftManagerController::new(committee_member);
            let mut last_error: Option<String> = None;

            for (i, run_data) in self.run_instance_data.iter().enumerate() {
                let height = run_data
                    .height
                    .map(|h| InstanceHeight::from(h as usize))
                    .unwrap_or_else(|| InstanceHeight::from(i));

                let value = run_data.input_value.clone().unwrap_or_default();
                if let Err(e) = controller.start_new_instance(height, value) {
                    last_error = Some(e);
                }

                let mut decided_count = 0;
                let empty_messages = vec![];
                let messages = run_data.input_messages.as_ref().unwrap_or(&empty_messages);

                for msg in messages.iter() {
                    match controller.process_msg(msg) {
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
                        last_error = Some("Decided count mismatch".to_string());
                    }
                }

                if let Ok(_root) = controller.get_root() {
                    // TODO: Compare with run_data.controller_post_root
                }
            }


            if !self.expected_error.is_empty() {
                last_error.is_some()
            } else {
                last_error.is_none()
            }
        })
    }

    fn test_type() -> SpecTestType {
        SpecTestType::Qbft(QbftSpecTestType::Controller)
    }
}
