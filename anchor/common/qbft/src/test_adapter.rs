use std::sync::Arc;

use crate::{Completed, Config, DefaultLeaderFunction, MessageId, MessageSender, Qbft, QbftData, Round, UnsignedWrappedQbftMessage, WrappedQbftMessage};
use types::Hash256;

/// Test message sender that collects sent messages
struct TestMessageSender<D: QbftData<Hash = Hash256>> {
    sent: Vec<UnsignedWrappedQbftMessage>,
}

impl<D: QbftData<Hash = Hash256>> MessageSender for TestMessageSender<D> {
    fn send(&mut self, msg: UnsignedWrappedQbftMessage) {
        self.sent.push(msg);
    }
}

/// QBFT Test Adapter for flexible testing
pub struct QbftTestAdapter<D: QbftData<Hash = Hash256>> {
    qbft: Qbft<DefaultLeaderFunction, D, TestMessageSender<D>>,
}

impl<D: QbftData<Hash = Hash256>> QbftTestAdapter<D> {
    /// Create a new test adapter
    pub fn new(config: Config<DefaultLeaderFunction>, start_data: D, identifier: MessageId) -> Self {
        let sender = TestMessageSender { sent: vec![] };
        let qbft = Qbft::new(config, start_data, identifier, sender);
        Self { qbft }
    }

    /// Create a proposal message
    pub fn create_proposal(&self, data: Arc<D>, round: Option<Round>) -> Result<UnsignedWrappedQbftMessage, Box<dyn std::error::Error>> {
        self.qbft.create_proposal(data, round)
    }

    /// Create a prepare message
    pub fn create_prepare(&self, data_hash: D::Hash, round: Option<Round>) -> Result<UnsignedWrappedQbftMessage, Box<dyn std::error::Error>> {
        self.qbft.create_prepare(data_hash, round)
    }

    /// Create a commit message
    pub fn create_commit(&self, data_hash: D::Hash, round: Option<Round>) -> Result<UnsignedWrappedQbftMessage, Box<dyn std::error::Error>> {
        self.qbft.create_commit(data_hash, round)
    }

    /// Create a round change message
    pub fn create_round_change(&self, state_value: Option<Vec<u8>>, target_round: Option<Round>) -> Result<UnsignedWrappedQbftMessage, Box<dyn std::error::Error>> {
        self.qbft.create_round_change(state_value, target_round)
    }

    /// Receive a message
    pub fn receive(&mut self, wrapped_msg: WrappedQbftMessage) {
        self.qbft.receive(wrapped_msg);
    }

    /// Get completed state
    pub fn completed(&self) -> Option<Completed<D>> {
        self.qbft.completed()
    }

    /// Take sent messages
    pub fn take_sent(&mut self) -> Vec<UnsignedWrappedQbftMessage> {
        std::mem::take(&mut self.qbft.message_sender.sent)
    }

    /// Set test state
    pub fn set_test_state(&mut self, last_prepared_round: Option<Round>, last_prepared_value: Option<D::Hash>) {
        self.qbft.set_test_state(last_prepared_round, last_prepared_value);
    }

    /// Add test data
    pub fn add_test_data(&mut self, data_hash: D::Hash, data: Arc<D>) {
        self.qbft.add_test_data(data_hash, data);
    }

    /// Add test justifications
    pub fn add_test_justifications(&mut self, round: Round, messages: Vec<WrappedQbftMessage>) {
        self.qbft.add_test_justifications(round, messages);
    }
}