use serde::Deserialize;
use ssv_types::msgid::{DutyExecutor, MessageId};

use crate::{
    SpecTest, SpecTestType, types::TypesSpecTestType, utils::test_keys::TESTING_VALIDATOR_PUBKEY,
};

#[derive(Debug, Deserialize)]
pub struct SSVMessageTest {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "MessageIDs")]
    pub message_ids: Vec<MessageId>,
    #[serde(rename = "BelongsToValidator")]
    pub belongs_to_validator: bool,
}

impl SpecTest for SSVMessageTest {
    fn name(&self) -> &str {
        &self.name
    }

    fn setup(&mut self) {
        // No-op
    }

    fn run(&self) -> bool {
        // Setup the 4 share set
        let mut result = true;
        for msg_id in &self.message_ids {
            println!("Processing MessageId: {:?}", msg_id);
            println!("Role: {:?}", msg_id.role());
            println!("DutyExecutor: {:?}", msg_id.duty_executor());

            // Some of message ids have an invalid role
            if let Some(duty_executor) = msg_id.duty_executor() {
                let validator_pubkey = match duty_executor {
                    DutyExecutor::Validator(key) => key,
                    _ => {
                        println!("DutyExecutor is not a Validator, returning false");
                        return false;
                    }
                };

                println!("Validator pubkey: {:?}", validator_pubkey);
                println!("Testing validator pubkey: {:?}", *TESTING_VALIDATOR_PUBKEY);

                if self.belongs_to_validator {
                    let matches = validator_pubkey == *TESTING_VALIDATOR_PUBKEY;
                    println!(
                        "Should belong to validator: {}, matches: {}",
                        self.belongs_to_validator, matches
                    );
                    result &= matches;
                } else {
                    let matches = validator_pubkey != *TESTING_VALIDATOR_PUBKEY;
                    println!(
                        "Should NOT belong to validator: {}, matches: {}",
                        self.belongs_to_validator, matches
                    );
                    result &= matches;
                }
            } else {
                println!("No duty executor found for MessageId");
            }
        }
        println!("Final result: {}", result);
        result
    }

    fn test_type() -> SpecTestType {
        SpecTestType::Types(TypesSpecTestType::SSVMsg)
    }
}
