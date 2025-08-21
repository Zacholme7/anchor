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
            // Check if TEST_FILTER env var is set to filter tests
            if let Ok(filter) = std::env::var("TEST_FILTER") {
                if !self.name.contains(&filter) {
                    println!("⏭️  SKIPPING TEST: {} (filter: {})", self.name, filter);
                    return true; // Skip this test but report as passing
                } else {
                    println!("▶️  RUNNING TEST: {} (matched filter: {})", self.name, filter);
                }
            }
            
            println!("🧪 RUNNING TEST: {}", self.name);
            println!("📖 Description: {}", self.documentation);


            let committee_member = self.controller.as_ref().unwrap().committee_member.clone();
            let mut controller = QbftManagerController::new(committee_member);
            let mut last_error: Option<String> = None;

            for (i, run_data) in self.run_instance_data.iter().enumerate() {
                let height = run_data
                    .height
                    .map(|h| InstanceHeight::from(h as usize))
                    .unwrap_or_else(|| InstanceHeight::from(i));

                println!("🎯 === Running instance {} at height {:?} ===", i, height);

                let value = run_data.input_value.clone().unwrap_or_default();
                println!("📝 Starting instance with {} bytes of input value", value.len());

                if let Err(e) = controller.start_new_instance(height, value).await {
                    println!("❌ Failed to start instance: {}", e);
                    last_error = Some(e);
                }

                let mut decided_count = 0;
                let empty_messages = vec![];
                let messages = run_data.input_messages.as_ref().unwrap_or(&empty_messages);

                println!("📨 Processing {} messages for instance {}", messages.len(), i);


                for (msg_idx, msg) in messages.iter().enumerate() {
                    println!("📩 Processing message {} of {}", msg_idx + 1, messages.len());

                    match controller.process_msg(msg).await {
                        Ok(Some(decided_data)) => {
                            decided_count += 1;
                            println!("✅ Message {} triggered decision #{} ({} bytes)",
                                     msg_idx + 1, decided_count, decided_data.len());

                            if let Some(expected) = &run_data.expected_decided_state {
                                if let Some(expected_bytes) = &expected.decided_value {
                                    if decided_data != *expected_bytes {
                                        println!("❌ Value mismatch: got {} bytes, expected {} bytes",
                                               decided_data.len(), expected_bytes.len());
                                        last_error = Some(format!("Decided value mismatch: got {} bytes, expected {} bytes",
                                                                decided_data.len(), expected_bytes.len()));
                                    } else {
                                        println!("✅ Decision value matches expected");
                                    }
                                }
                            }
                        }
                        Ok(None) => {
                            println!("⏳ Message {} - no decision yet", msg_idx + 1);
                        }
                        Err(e) => {
                            println!("❌ Message {} failed: {}", msg_idx + 1, e);
                            last_error = Some(e);
                        }
                    }
                }

                // Give the consensus tasks time to complete
                println!("⏳ Waiting for consensus to complete...");
                tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
                
                // Check if THIS specific instance has decided
                if controller.is_instance_decided(height) {
                    decided_count = 1;
                }

                println!("🏁 Instance {} complete: got {} decisions", i, decided_count);

                if let Some(expected) = &run_data.expected_decided_state {
                    println!("🎯 Expected {} decisions, got {}", expected.decided_count, decided_count);
                    if expected.decided_count != decided_count as u64 {
                        println!("❌ MISMATCH: expected {}, got {}", expected.decided_count, decided_count);
                        last_error = Some("Decided count mismatch".to_string());
                    } else {
                        println!("✅ Decision count matches expected");
                    }
                } else {
                    println!("ℹ️  No expected decision state for instance {}", i);
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
