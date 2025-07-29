use qbft::InstanceHeight;
use qbft::{DefaultLeaderFunction, Qbft, UnsignedWrappedQbftMessage};
use ssv_types::consensus::{BeaconVote, QbftMessageType};
use ssv_types::message::SignedSSVMessage;

use types::Hash256;

/// State that we want to initialize the qbft instance with
pub struct QbftStartingState {
    height: Option<InstanceHeight>,
}

// Simple mock handler type
type MockHandler = Box<dyn FnMut(UnsignedWrappedQbftMessage)>;

// Adapter over our core qbft instance
pub struct QbftAdapter {
    instance: Option<Qbft<DefaultLeaderFunction, BeaconVote, MockHandler>>,
}

impl QbftAdapter {
    /// Build a QBFT instance with starting state
    pub fn new_with_state(_state: QbftStartingState) -> Self {
        let mock_handler: MockHandler = Box::new(|_msg| {
            // Mock implementation - just ignore the message
        });

        Self {
            instance: None, // TODO: Initialize with actual Qbft instance using mock_handler
        }
    }

    /// Create a new SignedSSVMessage using the instance
    pub fn create_message(
        &self,
        msg_type: QbftMessageType,
        data_hash: Hash256,
        rc_justifications: &Option<Vec<SignedSSVMessage>>,
        pre_justifications: &Option<Vec<SignedSSVMessage>>,
        round: Option<u64>,
    ) -> Result<SignedSSVMessage, bool> {
        /*
                let res = self.instance.new_unsigned_message_spec(
                    msg_type,
                    data_hash,
                    rc_justifications,
                    rc_justifications,
                    round,
                );
        */
        // call into the new_unsigned_msg in the qbft instance
        todo!()
    }
}

// Simple mock function factory
pub fn create_mock_handler() -> impl FnMut(UnsignedWrappedQbftMessage) {
    |_msg: UnsignedWrappedQbftMessage| {
        // Mock implementation - just ignore the message
    }
}
