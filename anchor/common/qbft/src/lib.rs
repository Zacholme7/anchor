use crate::msg_container::MessageContainer;
use ssv_types::consensus::{Data, QbftMessage, QbftMessageType, UnsignedSSVMessage};
use ssv_types::message::{MessageID, MsgType, SSVMessage};
use ssv_types::msgid::MsgId;
use ssv_types::OperatorId;
use ssz::Encode;
use std::collections::HashMap;
use tracing::{debug, error, warn};
use types::Hash256;

// Re-Exports for Manager
pub use config::{Config, ConfigBuilder};
pub use error::ConfigBuilderError;
pub use qbft_types::Message;
pub use qbft_types::WrappedQbftMessage;
pub use qbft_types::{
    Completed, ConsensusData, DefaultLeaderFunction, InstanceHeight, InstanceState, LeaderFunction,
    Round,
};
pub use validation::{validate_consensus_data, ValidatedData, ValidationError};

mod config;
mod error;
mod msg_container;
mod qbft_types;
mod validation;

struct ConsensusRecord {
    round: Round,
    hash: Hash256,
    prepare_messages: Vec<WrappedQbftMessage>,
}

#[cfg(test)]
mod tests;

/// The structure that defines the Quorum Based Fault Tolerance (QBFT) instance.
///
/// This builds and runs an entire QBFT process until it completes. It can complete either
/// successfully (i.e that it has successfully come to consensus, or through a timeout where enough
/// round changes have elapsed before coming to consensus.
///
/// The QBFT instance will recieve SignedSSVMessages from the network and it will construct
/// UnsignedSSVMessages to be signed and sent on the network.
pub struct Qbft<F, D, S>
where
    F: LeaderFunction + Clone,
    D: Data + Encode,
    S: FnMut(Message),
{
    /// The initial configuration used to establish this instance of QBFT.
    config: Config<F>,
    /// The identification of this QBFT instance
    identifier: MsgId,
    /// The instance height acts as an ID for the current instance and helps distinguish it from
    /// other instances.
    instance_height: InstanceHeight,

    /// Hash of the start data
    start_data_hash: D::Hash,
    /// Initial data that we will propose if we are the leader.
    start_data: D,
    /// All of the data that we have seen
    data: HashMap<D::Hash, Vec<u8>>,
    /// The current round this instance state is in.a
    current_round: Round,
    /// The current state of the instance
    state: InstanceState,
    /// If this QBFT instance has been completed, the completed value
    completed: Option<Completed<D::Hash>>,

    // Message containers
    propose_container: MessageContainer<WrappedQbftMessage>,
    prepare_container: MessageContainer<WrappedQbftMessage>,
    commit_container: MessageContainer<WrappedQbftMessage>,
    round_change_container: MessageContainer<WrappedQbftMessage>,

    // Current round state
    proposal_accepted_for_current_round: Option<WrappedQbftMessage>,
    last_prepared_round: Option<Round>,
    last_prepared_value: Option<D::Hash>,

    /// Past prepare consensus that we have reached
    past_consensus: HashMap<Round, ConsensusRecord>,

    // Network sender
    send_message: S,
}

impl<F, D, S> Qbft<F, D, S>
where
    F: LeaderFunction + Clone,
    D: Data<Hash = Hash256> + Encode,
    S: FnMut(Message),
{
    // Construct a new QBFT Instance and start the first round
    pub fn new(config: Config<F>, start_data: D, send_message: S) -> Self {
        let instance_height = *config.instance_height();
        let current_round = config.round();
        let quorum_size = config.quorum_size();

        let mut qbft = Qbft {
            config,
            identifier: MsgId([0; 56]),
            instance_height,

            start_data_hash: start_data.hash(),
            start_data,
            data: HashMap::new(),
            current_round,
            state: InstanceState::AwaitingProposal,
            completed: None,

            propose_container: MessageContainer::new(quorum_size),
            prepare_container: MessageContainer::new(quorum_size),
            commit_container: MessageContainer::new(quorum_size),
            round_change_container: MessageContainer::new(quorum_size),

            proposal_accepted_for_current_round: None,
            last_prepared_round: None,
            last_prepared_value: None,

            past_consensus: HashMap::new(),

            send_message,
        };
        qbft.start_round();
        qbft
    }

    // Hash of the start data
    pub fn start_data_hash(&self) -> &D::Hash {
        &self.start_data_hash
    }

    /// Return a reference to the qbft configuration
    pub fn config(&self) -> &Config<F> {
        &self.config
    }

    // Shifts this instance into a new round>
    fn set_round(&mut self, new_round: Round) {
        self.current_round.set(new_round);
        self.start_round();
    }

    // Validation and check functions.
    fn check_leader(&self, operator_id: &OperatorId) -> bool {
        self.config.leader_fn().leader_function(
            operator_id,
            self.current_round,
            self.instance_height,
            self.config.committee_members(),
        )
    }

    /// Checks to make sure any given operator is in this instance's comittee.
    fn check_committee(&self, operator_id: &OperatorId) -> bool {
        self.config.committee_members().contains(operator_id)
    }

    // Perform base QBFT relevant message verification. This verfiication is applicable to all QBFT
    // message types
    fn validate_message(&self, wrapped_msg: &WrappedQbftMessage) -> bool {
        // Validate the wrapped message. This will validate the SignedSsvMessage and the QbftMessage
        if !wrapped_msg.validate() {
            warn!("Message validation unsuccessful");
            return false;
        }

        // Ensure that this message is for the correct round
        let current_round = self.current_round.get();
        if (wrapped_msg.qbft_message.round < current_round as u64)
            || (current_round > self.config.max_rounds())
        {
            warn!(
                propose_round = wrapped_msg.qbft_message.round,
                current_round = *self.current_round,
                "Message received for a invalid round"
            );
            return false;
        }

        // Make sure there is only one signer
        if wrapped_msg.signed_message.operator_ids().len() != 1 {
            warn!(
                num_signers = wrapped_msg.signed_message.operator_ids().len(),
                "Propose message only allows one signer"
            );
            return false;
        }

        // Make sure we are at the correct instance height
        if wrapped_msg.qbft_message.height != *self.instance_height as u64 {
            warn!(
                expected_instance = *self.instance_height,
                "Message received for the wrong instance"
            );
            return false;
        }

        // Success! Message is well formed
        true
    }

    /// Justify the round change quorum
    /// In order to justify a round change quorum, we find the maximum round of the quorum set that
    /// had achieved a past consensus. If we have also seen consensus on this round for the
    /// suggested data, then it is justified and this function returns that data.
    /// If there is no past consensus data in the round change quorum or we disagree with quorum set
    /// this function will return None, and we obtain the data as if we were beginning this
    /// instance.
    fn justify_round_change_quorum(&self) -> Option<(Hash256, Vec<u8>)> {
        // Get all round change messages for the current round
        let round_change_messages = self
            .round_change_container
            .get_messages_for_round(self.current_round);

        // If we don't have enough messages for quorum, we can't justify anything
        if round_change_messages.len() < self.config.quorum_size() {
            return None;
        }

        // Find the highest round that any node claims reached preparation
        let highest_prepared = round_change_messages
            .iter()
            .filter(|msg| msg.qbft_message.data_round != 0) // Only consider messages with prepared data
            .max_by_key(|msg| msg.qbft_message.data_round);

        // If we found a message with prepared data
        if let Some(highest_msg) = highest_prepared {
            // Get the prepared data from the message
            let prepared_data = highest_msg.signed_message.full_data();
            let prepared_round = Round::from(highest_msg.qbft_message.data_round);

            // Verify we have also seen this consensus
            if let Some(our_record) = self.past_consensus.get(&prepared_round) {
                // We have seen consensus on the data, get the value
                let our_data = self
                    .data
                    .get(&our_record.hash)
                    .expect("Data must exist")
                    .clone();

                // Verify the data matches what we saw
                if prepared_data == our_data {
                    // We agree with the prepared data - use it
                    return Some((our_record.hash, our_data));
                }
            }
        }

        // No consensus found or we disagree - use initial data
        None
    }

    // Handles the beginning of a round.
    fn start_round(&mut self) {
        debug!(round = *self.current_round, "Starting new round");

        // Initialise the instance state for the round
        self.state = InstanceState::AwaitingProposal;

        // Check if we are the leader
        if self.check_leader(&self.config.operator_id()) {
            // We are the leader
            debug!("Current leader");

            // Check justification of round change quorum. If there is a justification, we will use
            // that data. Otherwise, use the initial state data
            let (data_hash, data) = self
                .justify_round_change_quorum()
                .unwrap_or_else(|| (self.start_data_hash, self.start_data.as_ssz_bytes()));

            // Send the initial proposal
            self.send_proposal(data_hash, data);
        }
    }

    // Receive a new message from the network
    pub fn receive(&mut self, wrapped_msg: WrappedQbftMessage) {
        // Perform base qbft releveant verification on the message
        if !self.validate_message(&wrapped_msg) {
            return;
        }

        // We know where is only one signer, so the first (and only) operator in the signed message
        // is the sender
        let operator_id = wrapped_msg
            .signed_message
            .operator_ids()
            .first()
            .expect("Confirmed to exist in validation");
        let operator_id = OperatorId(*operator_id);

        // Check that this sender is in our committee
        if !self.check_committee(&operator_id) {
            warn!(
                from = ?operator_id,
                "PROPOSE message from non-committee operator"
            );
            return;
        }
        let msg_round: Round = wrapped_msg.qbft_message.round.into();

        // All basic verification successful! Dispatch to the correct handler
        match wrapped_msg.qbft_message.qbft_message_type {
            QbftMessageType::Proposal => self.received_propose(operator_id, msg_round, wrapped_msg),
            QbftMessageType::Prepare => self.received_prepare(operator_id, msg_round, wrapped_msg),
            QbftMessageType::Commit => self.received_commit(operator_id, msg_round, wrapped_msg),
            QbftMessageType::RoundChange => {
                self.received_round_change(operator_id, msg_round, wrapped_msg)
            }
        }
    }

    // We have received a new Proposal messaage
    fn received_propose(
        &mut self,
        operator_id: OperatorId,
        round: Round,
        wrapped_msg: WrappedQbftMessage,
    ) {
        // Make sure that we are actually waiting for a proposal
        if !matches!(self.state, InstanceState::AwaitingProposal) {
            warn!(from=?operator_id, ?self.state, "PROPOSE message while in invalid state");
            return;
        }

        // Check if proposal is from the leader we expect
        if !self.check_leader(&operator_id) {
            warn!(from = ?operator_id, "PROPOSE message from non-leader");
            return;
        }

        // Round change justification validation for rounds after the first
        if round > Round::default() {
            //self.validate_round_change_justification();
        }

        // Validate the prepare justifications if they exist
        if !wrapped_msg.qbft_message.prepare_justification.is_empty() {
            //self.validate_prepare_justification(wrapped_msg)?;
        }

        // Verify that the fulldata matches the data root of the qbft message
        let data_hash = wrapped_msg.signed_message.hash_fulldata();
        if data_hash != wrapped_msg.qbft_message.root {
            warn!(from = ?operator_id, "Data roots do not match");
            return;
        }

        debug!(from = ?operator_id, "PROPOSE received");

        // Store the received propse message
        if !self
            .propose_container
            .add_message(round, operator_id, &wrapped_msg)
        {
            warn!(from = ?operator_id, "PROPOSE message is a duplicate")
        }

        // Store the data
        self.data.insert(
            data_hash,
            wrapped_msg.signed_message.full_data().to_vec().clone(),
        );

        // Update state
        self.proposal_accepted_for_current_round = Some(wrapped_msg);
        self.state = InstanceState::Prepare;

        // Create and send prepare message
        self.send_prepare(data_hash);
    }

    /// We have received a prepare message
    fn received_prepare(
        &mut self,
        operator_id: OperatorId,
        round: Round,
        wrapped_msg: WrappedQbftMessage,
    ) {
        // Check that we are in the correct state
        if (self.state as u8) >= (InstanceState::SentRoundChange as u8) {
            warn!(from=?operator_id, ?self.state, "PREPARE message while in invalid state");
            return;
        }

        debug!(from = ?operator_id, "PREPARE received");

        // Store the prepare message
        if !self
            .prepare_container
            .add_message(round, operator_id, &wrapped_msg)
        {
            warn!(from = ?operator_id, "PREPARE message is a duplicate")
        }

        // Check if we have reached quorum, if so send the commit message
        if let Some(hash) = self.prepare_container.has_quorum(round) {
            // Make sure we are in the correct state
            if !matches!(self.state, InstanceState::Prepare) {
                warn!(from=?operator_id, ?self.state, "Not in PREPARE state");
                return;
            }

            // Move the state forward since we have a prepare quorum
            self.state = InstanceState::Commit;

            // Record this prepare consensus. Fetch all of the preapre messages for the data and then
            // record the consensus for them
            let prepare_messages: Vec<WrappedQbftMessage> = self
                .prepare_container
                .get_messages_for_value(round, hash)
                .into_iter()
                .cloned()
                .collect();

            let record = ConsensusRecord {
                round,
                hash,
                prepare_messages,
            };
            self.past_consensus.insert(round, record);

            // Send a commit message for the prepare quorum data
            self.send_commit(hash);
        }
    }

    /// We have received a commit message
    fn received_commit(
        &mut self,
        operator_id: OperatorId,
        round: Round,
        wrapped_msg: WrappedQbftMessage,
    ) {
        // Make sure that we are in the correct state
        if (self.state as u8) >= (InstanceState::SentRoundChange as u8) {
            warn!(from=*operator_id, ?self.state, "COMMIT message while in invalid state");
            return;
        }

        debug!(from = ?operator_id, "COMMIT received");

        // Store the received commit message
        if !self
            .commit_container
            .add_message(round, operator_id, &wrapped_msg)
        {
            warn!(from = ?operator_id, "COMMIT message is a duplicate")
        }

        // Check if we have a commit quorum
        if let Some(hash) = self.prepare_container.has_quorum(round) {
            if matches!(self.state, InstanceState::Commit) {
                // We have come to consensus, mark ourself as completed and record the agreed upon
                // value
                self.state = InstanceState::Complete;
                self.completed = Some(Completed::Success(hash));
            }
        }
    }

    /// We have received a round change message.
    fn received_round_change(
        &mut self,
        operator_id: OperatorId,
        round: Round,
        wrapped_msg: WrappedQbftMessage,
    ) {
        // Make sure we are in the correct state
        if (self.state as u8) >= (InstanceState::Complete as u8) {
            warn!(from=*operator_id, ?self.state, "ROUNDCHANGE message while in invalid state");
            return;
        }

        debug!(from = ?operator_id, "ROUNDCHANGE received");

        // Store the round changed message
        if !self
            .round_change_container
            .add_message(round, operator_id, &wrapped_msg)
        {
            warn!(from = ?operator_id, "ROUNDCHANGE message is a duplicate")
        }

        // There are two cases to check here
        // 1. If we have received a quorum of round change messages, we need to start a new round
        // 2. If we receive f+1 round change messages, we need to send our own round-change message

        // Check if we have any messages for the suggested round
        if let Some(hash) = self.round_change_container.has_quorum(round) {
            if matches!(self.state, InstanceState::SentRoundChange) {
                // 1. If we have reached a quorum for this round, advance to that round.
                debug!(
                    operator_id = ?self.config.operator_id(),
                    round = *round,
                    "Round change quorum reached"
                );
                self.set_round(round);
            } else {
                let num_messages_for_round =
                    self.round_change_container.num_messages_for_round(round);
                if num_messages_for_round > self.config.get_f()
                    && !(matches!(self.state, InstanceState::SentRoundChange))
                {
                    self.send_round_change(hash);
                }
            }
        }
    }

    // End the current round and move to the next one, if possible.
    pub fn end_round(&mut self) {
        debug!(round = *self.current_round, "Incrementing round");
        let Some(next_round) = self.current_round.next() else {
            self.state = InstanceState::Complete;
            self.completed = Some(Completed::TimedOut);
            return;
        };
        if next_round.get() > self.config.max_rounds() {
            self.state = InstanceState::Complete;
            self.completed = Some(Completed::TimedOut);
            return;
        }

        // Get the data to send with the round change?
        self.send_round_change(self.start_data_hash);
        // Start a new round
        self.set_round(next_round);
    }

    // Construct a new unsigned message. This will be passed to the processor to be signed and then
    // sent on the network
    fn new_unsigned_message(
        &self,
        msg_type: QbftMessageType,
        data_hash: D::Hash,
    ) -> UnsignedSSVMessage {
        // Create the QBFT message
        let _qbft_mesage = QbftMessage {
            qbft_message_type: msg_type,
            height: *self.instance_height as u64,
            round: self.current_round.get() as u64,
            identifier: self.identifier.clone(),
            root: data_hash as Hash256,
            data_round: self.current_round.get() as u64,
            round_change_justification: vec![], // Empty for MVP
            prepare_justification: vec![],      // Empty for MVP
        };

        let ssv_message = SSVMessage::new(
            MsgType::SSVConsensusMsgType,
            MessageID::new([0; 56]),
            vec![], // qbft_message.as_ssz_bytes()
        );

        // Wrap in unsigned SSV message
        UnsignedSSVMessage {
            ssv_message,
            full_data: self.data.get(&data_hash).unwrap().clone(),
        }
    }

    // Send a new qbft proposal message
    fn send_proposal(&mut self, hash: D::Hash, data: Vec<u8>) {
        // Store the data we're proposing
        self.data.insert(hash, data.clone());

        // Construct a unsigned proposal
        let unsigned_msg = self.new_unsigned_message(QbftMessageType::Proposal, hash);

        let operator_id = self.config.operator_id();
        (self.send_message)(Message::Propose(operator_id, unsigned_msg.clone()));
    }

    // Send a new qbft prepare message
    fn send_prepare(&mut self, data_hash: D::Hash) {
        // Only send prepare if we've seen this data
        if !self.data.contains_key(&data_hash) {
            warn!("Attempted to prepare unknown data");
            return;
        }

        // Construct unsigned prepare
        let unsigned_msg = self.new_unsigned_message(QbftMessageType::Prepare, data_hash);

        let operator_id = self.config.operator_id();
        (self.send_message)(Message::Prepare(operator_id, unsigned_msg.clone()));
    }

    // Send a new qbft commit message
    fn send_commit(&mut self, data_hash: D::Hash) {
        // Construct unsigned commit
        let unsigned_msg = self.new_unsigned_message(QbftMessageType::Commit, data_hash);

        let operator_id = self.config.operator_id();
        (self.send_message)(Message::Commit(operator_id, unsigned_msg.clone()));
    }

    // Send a new qbft round change message
    fn send_round_change(&mut self, data_hash: D::Hash) {
        // Construct unsigned round change
        let unsigned_msg = self.new_unsigned_message(QbftMessageType::RoundChange, data_hash);

        let operator_id = self.config.operator_id();
        (self.send_message)(Message::RoundChange(operator_id, unsigned_msg.clone()));
    }

    /// Extract the data that the instance has come to consensus on
    pub fn completed(&self) -> Option<Completed<Vec<u8>>> {
        self.completed
            .clone()
            .and_then(|completed| match completed {
                Completed::TimedOut => Some(Completed::TimedOut),
                Completed::Success(hash) => {
                    let data = self.data.get(&hash).cloned();
                    if data.is_none() {
                        error!("could not find finished data");
                    }
                    data.map(Completed::Success)
                }
            })
    }
}
