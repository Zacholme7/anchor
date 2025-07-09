use openssl::pkey::{PKey, Private};
use serde::Deserialize;
use ssv_types::{IndexSet, OperatorId, Round, consensus::QbftMessageType, msgid::MessageId};
use types::Hash256;

use super::{SpecQbft, qbft_deserializers::*};
use crate::{
    QbftSpecTestType, SpecTest, SpecTestType, qbft::SignedSSVMessage, utils::test_keys::TestKeySet,
};

impl SpecTest for CreateMessageTest {
    fn name(&self) -> &str {
        &self.name
    }

    // Run the test by constructing the message and verifying its correctness
    fn run(&self) -> bool {
        let spec_qbft = self.spec_qbft.as_ref().expect("Setup has been called");
        let key = self.signing_key.as_ref().expect("Setup has been called");
        
        // Validate justification data first
        if let Err(_validation_error) = self.validate_justification_data() {
            return false;
        }
        let prepare_justifications = if let Some(prepare) = &self.prepare_justifications {
            prepare.clone()
        } else {
            Vec::new()
        };
        let round_change_justifications =
            if let Some(round_change) = &self.round_change_justifications {
                round_change.clone()
            } else {
                Vec::new()
            };

        // Create a new unsigned message. Have to create a new unsigned message to be received on
        // the queue and then perform signing
        let unsigned_message = spec_qbft.create_message_with_state_value(
            self.create_type,
            self.root,
            self.round,
            round_change_justifications.clone(),
            prepare_justifications.clone(),
            self.state_value.as_deref(),
        );

        let signed_message = spec_qbft.sign(unsigned_message, key);
        
        // Compute the merkle root of the message and compare it to the expected_root
        spec_qbft.verify_root(signed_message, self.expected_root)
    }

    // Setup the qbft instance for constructing a new message
    fn setup(&mut self) {
        let four_share_set = TestKeySet::four_share_set();
        let committee: IndexSet<OperatorId> =
            four_share_set.operator_keys.keys().cloned().collect();

        // All test identifiers are [1,2,3,4]
        let identifier = MessageId::for_spectest();

        // All message creation testing code uses operator one as the message signer
        let operator_one_private = four_share_set
            .operator_keys
            .get(&OperatorId::from(1))
            .expect("Exists");
        let operator_one_private =
            PKey::from_rsa(operator_one_private.to_owned()).expect("Valid key");

        let qbft = SpecQbft::new(committee, identifier);

        // Complete the setup
        self.spec_qbft = Some(qbft);
        self.signing_key = Some(operator_one_private);
    }

    fn test_type() -> SpecTestType {
        SpecTestType::Qbft(QbftSpecTestType::CreateMessage)
    }
}

// Representation of CreateMsgSpecTest files
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateMessageTest {
    // Name of the test that is being run
    #[serde(rename = "Name")]
    pub name: String,

    // Root of the QBFT Message, This is the unhashed ssz bytes of the data
    #[serde(rename = "Value", deserialize_with = "deserialize_value_into_root")]
    pub root: Hash256,

    // The last prepared value of the qbft instance. Todo!() What format is this in??
    #[serde(rename = "StateValue")]
    pub state_value: Option<String>,

    // The round this message is for
    #[serde(rename = "Round", deserialize_with = "deserialize_u64_into_round")]
    pub round: Option<Round>,

    // Any round change justifications for the message
    #[serde(rename = "RoundChangeJustifications")]
    pub round_change_justifications: Option<Vec<SignedSSVMessage>>,

    // Any prepare justifications for the message
    #[serde(rename = "PrepareJustifications")]
    pub prepare_justifications: Option<Vec<SignedSSVMessage>>,

    // The type of the QBFT Message to create
    #[serde(
        rename = "CreateType",
        deserialize_with = "deserialize_qbft_message_type"
    )]
    pub create_type: QbftMessageType,

    // The Expected Root of the QBFT Message
    #[serde(rename = "ExpectedRoot")]
    pub expected_root: Hash256,

    // Any Errors that were expected
    #[serde(rename = "ExpectedError")]
    pub expected_error: String,

    // Qbft Instance that is used for running the test. Skip this during deserialization
    #[serde(skip)]
    pub spec_qbft: Option<SpecQbft>,

    // The operator private key for message signing
    #[serde(skip)]
    pub signing_key: Option<PKey<Private>>,
}

impl CreateMessageTest {

    /// Validate that justifications have expected format
    fn validate_justification_data(&self) -> Result<(), String> {
        // Validate round change justifications
        if let Some(ref round_change_justifications) = self.round_change_justifications {
            for (i, justification) in round_change_justifications.iter().enumerate() {
                if justification.signatures().is_empty() {
                    return Err(format!("Round change justification {i} has no signatures"));
                }
                if justification.operator_ids().is_empty() {
                    return Err(format!("Round change justification {i} has no operator IDs"));
                }
                
                // Check if full_data is base64 decodable (if not empty)
                if !justification.full_data().is_empty() {
                    let full_data = justification.full_data();
                    if full_data.len() > 100 { // Reasonable max size check
                        return Err(format!("Round change justification {i} has unexpectedly large full_data: {} bytes", full_data.len()));
                    }
                }
            }
        }
        
        // Validate prepare justifications
        if let Some(ref prepare_justifications) = self.prepare_justifications {
            for (i, justification) in prepare_justifications.iter().enumerate() {
                if justification.signatures().is_empty() {
                    return Err(format!("Prepare justification {i} has no signatures"));
                }
                if justification.operator_ids().is_empty() {
                    return Err(format!("Prepare justification {i} has no operator IDs"));
                }
                
                // Check if full_data is reasonable
                if !justification.full_data().is_empty() {
                    let full_data = justification.full_data();
                    if full_data.len() > 100 { // Reasonable max size check
                        return Err(format!("Prepare justification {i} has unexpectedly large full_data: {} bytes", full_data.len()));
                    }
                }
            }
        }
        
        Ok(())
    }

}
