use std::{collections::HashMap, sync::Arc};

// Re-Exports for Manager
pub use config::{Config, ConfigBuilder};
pub use error::{ConfigBuilderError, QbftError};
pub use qbft_types::{
    Completed, ConsensusData, DefaultLeaderFunction, InstanceHeight, InstanceState, LeaderFunction,
    UnsignedWrappedQbftMessage, WrappedQbftMessage,
};
use sha2::{Digest, Sha256};
use ssv_types::{
    OperatorId, Round, VariableList,
    consensus::{QbftData, QbftMessage, QbftMessageType, UnsignedSSVMessage},
    message::{MsgType, SSVMessage, SignedSSVMessage},
    msgid::MessageId,
};
use ssz::Encode;
use tracing::{debug, error, warn};
use types::{FixedBytesExtended, Hash256};

use crate::msg_container::MessageContainer;

mod config;
mod error;
mod justification;
mod msg_container;
mod qbft_types;

#[cfg(test)]
mod tests;

// Internal structure to hold the data that is to be included in a new outgoing message
struct MessageData<D: QbftData<Hash = Hash256>> {
    data_round: u64,
    round: u64,
    root: D::Hash,
    full_data: Vec<u8>,
}

impl<D: QbftData<Hash = Hash256>> MessageData<D> {
    pub fn new(data_round: u64, round: u64, root: D::Hash, full_data: Vec<u8>) -> Self {
        Self {
            data_round,
            round,
            root,
            full_data,
        }
    }
}

// Store hash and deserialized data together to avoid redundant lookups
#[derive(Debug, Default, Clone)]
pub struct ValidData<D: QbftData<Hash = Hash256>> {
    hash: D::Hash,
    data: Option<Arc<D>>,
}

impl<D: QbftData<Hash = Hash256>> ValidData<D> {
    fn new(data: Option<Arc<D>>, hash: Hash256) -> Self {
        Self { hash, data }
    }
}

pub trait MessageSender {
    fn send(&mut self, msg: UnsignedWrappedQbftMessage);
}

impl<T: FnMut(UnsignedWrappedQbftMessage)> MessageSender for T {
    fn send(&mut self, msg: UnsignedWrappedQbftMessage) {
        self(msg)
    }
}

/// The structure that defines the Quorum Based Fault Tolerance (QBFT) instance.
///
/// This builds and runs an entire QBFT process until it completes. It can complete either
/// successfully (i.e that it has successfully come to consensus, or through a timeout where enough
/// round changes have elapsed before coming to consensus.
///
/// The QBFT instance will receive WrappedQbftMessages from the network and it will construct
/// UnsignedSSVMessages to be signed and sent on the network.
pub struct Qbft<F, D, S>
where
    F: LeaderFunction + Clone,
    D: QbftData<Hash = Hash256>,
    S: MessageSender,
{
    /// The initial configuration used to establish this instance of QBFT.
    config: Config<F>,
    /// The identification of this QBFT instance
    identifier: MessageId,
    /// The instance height acts as an ID for the current instance and helps distinguish it from
    /// other instances.
    instance_height: InstanceHeight,
    /// Hash of the start data
    start_data_hash: D::Hash,
    /// Initial data that we will propose if we are the leader.
    start_data: Arc<D>,
    /// Validated start data
    valid_start_data: ValidData<D>,
    /// All of the data that we have seen
    data: HashMap<D::Hash, Arc<D>>,
    /// The current round this instance state is in.
    current_round: Round,
    /// The current state of the instance
    state: InstanceState,
    /// If this QBFT instance has been completed, the completed value
    completed: Option<Completed<D::Hash>>,

    // Message containers
    propose_container: MessageContainer,
    prepare_container: MessageContainer,
    commit_container: MessageContainer,
    round_change_container: MessageContainer,

    // Current round state
    proposal_accepted_for_current_round: bool,
    proposal_root: Option<D::Hash>,
    last_prepared_round: Option<Round>,
    last_prepared_value: Option<D::Hash>,

    /// Past prepare consensus that we have reached
    past_consensus: HashMap<Round, D::Hash>,

    /// Aggregated commit message
    aggregated_commit: Option<SignedSSVMessage>,

    /// Message sender callback to instruct managing code to send a message
    message_sender: S,
}

impl<F, D, S> Qbft<F, D, S>
where
    F: LeaderFunction + Clone,
    D: QbftData<Hash = Hash256>,
    S: MessageSender,
{
    /// Constructs a new QBFT instance and starts the first round.
    ///
    /// # Parameters
    /// - `config`: The initial configuration used to establish this QBFT instance.
    /// - `start_data`: The initial data that will be proposed if this node is the leader.
    /// - `identifier`: The message identifier for this QBFT instance's outgoing messages.
    /// - `message_sender`: A callback used by the instance to trigger message sending.
    pub fn new(config: Config<F>, start_data: D, identifier: MessageId, message_sender: S) -> Self {
        let instance_height = *config.instance_height();
        let current_round = config.round();
        let quorum_size = config.quorum_size();

        let start_data = Arc::new(start_data);
        let start_data_hash = start_data.hash();
        let valid_start_data = ValidData::new(Some(start_data.clone()), start_data_hash);

        let mut qbft = Qbft {
            config,
            identifier,
            instance_height,

            start_data_hash,
            start_data,
            valid_start_data,
            data: HashMap::new(),
            current_round,
            state: InstanceState::AwaitingProposal,
            completed: None,

            propose_container: MessageContainer::new(quorum_size),
            prepare_container: MessageContainer::new(quorum_size),
            commit_container: MessageContainer::new(quorum_size),
            round_change_container: MessageContainer::new(quorum_size),

            proposal_accepted_for_current_round: false,
            proposal_root: None,
            last_prepared_round: None,
            last_prepared_value: None,

            past_consensus: HashMap::new(),

            aggregated_commit: None,

            message_sender,
        };
        qbft.data
            .insert(qbft.start_data_hash, qbft.start_data.clone());

        #[cfg(not(test))]
        //qbft.start_round();
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

    /// Get the current round
    pub fn get_round(&self) -> Round {
        self.current_round
    }

    // Shifts this instance into a new round>
    fn set_round(&mut self, new_round: Round) {
        self.current_round.set(new_round);
        self.start_round();
    }

    // Get the aggregated commit message, if it exists
    pub fn get_aggregated_commit(&self) -> Option<SignedSSVMessage> {
        self.aggregated_commit.clone()
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
    // Return type expresses that we either have
    // 1) An invalid message via None
    // 2) A valid message with empty fulldata via Some(None, ID)
    // 3) A valid message with fulldata via Some(data, ID)
    fn validate_message(
        &self,
        wrapped_msg: &WrappedQbftMessage,
    ) -> Result<(Option<ValidData<D>>, OperatorId), QbftError> {
        // Ensure that this message is for the correct round
        if wrapped_msg.qbft_message.round < self.current_round.into() {
            debug!(
                message_round = wrapped_msg.qbft_message.round,
                current_round = *self.current_round,
                "Message received for a previous round"
            );
            println!("for a past round");
            return Err(QbftError::PastRound);
        }

        // Check for future round
        if wrapped_msg.qbft_message.round > self.current_round.into() {
            match wrapped_msg.qbft_message.qbft_message_type {
                QbftMessageType::Proposal | QbftMessageType::RoundChange => {
                    // Proposals & Round Changes for future rounds are always allowed
                }
                QbftMessageType::Commit => {
                    // Only decided messages (with quorum) are allowed from future rounds
                    if wrapped_msg.signed_message.operator_ids().len() < self.config.quorum_size() {
                        return Err(QbftError::WrongRound);
                    }
                }
                _ => {
                    // All other message types (including Prepare) for future rounds are not allowed
                    return Err(QbftError::WrongRound);
                }
            }
        }

        // Make sure we are at the correct instance height
        if wrapped_msg.qbft_message.height != *self.instance_height as u64 {
            warn!(
                expected_instance = *self.instance_height,
                "Message received for the wrong instance"
            );
            return Err(QbftError::WrongHeight);
        }

        // Make sure that all of the signers are in our committee
        for signer in wrapped_msg.signed_message.operator_ids() {
            if !self.check_committee(signer) {
                warn!("Signer is not part of committee");
                return Err(QbftError::SignerNotInCommittee);
            }
        }

        // The rest of the verification only pertains to messages with one signature
        if wrapped_msg.signed_message.operator_ids().len() > 1 {
            // Multi-signer messages are ONLY allowed for COMMIT type
            // todo!() checked by msg validator
            if let QbftMessageType::Commit = wrapped_msg.qbft_message.qbft_message_type {
                // Multi-signer commits (decide messages) are only valid if they include us
                if !wrapped_msg
                    .signed_message
                    .operator_ids()
                    .contains(&self.config.operator_id())
                {
                    // This is a multi-signer commit that doesn't include us - invalid
                    return Err(QbftError::MultipleSignersNotAllowed);
                }
                let valid_data = Some(ValidData::new(None, wrapped_msg.qbft_message.root));
                return Ok((valid_data, OperatorId::from(0)));
            } else {
                // Multi-signer messages for non-COMMIT types are not allowed
                return Err(QbftError::MultipleSignersNotAllowed);
            }
        }

        // Message is not a decide message, we know there is only one signer
        let signer = wrapped_msg
            .signed_message
            .operator_ids()
            .first()
            .ok_or(QbftError::MissingOperators)?;

        // Fulldata may be empty. This is still considered valid though
        if wrapped_msg.signed_message.full_data().is_empty() {
            let valid_data = Some(ValidData::new(None, wrapped_msg.qbft_message.root));
            return Ok((valid_data, *signer));
        }

        // Try to decode the data. If we can decode the data, then also validate it
        let data = match D::from_ssz_bytes(wrapped_msg.signed_message.full_data()) {
            Ok(data) => data,
            _ => {
                error!(
                    msg = %wrapped_msg,
                    "Invalid full data received",
                );
                debug!(
                    full_data = hex::encode(wrapped_msg.signed_message.full_data()),
                    "Raw invalid full data",
                );
                return Err(QbftError::InvalidFullData);
            }
        };

        if !data.validate() {
            error!("Data failed validation");
            return Err(QbftError::DataValidationFailed);
        }

        // Success! Message is well formed
        let valid_data = Some(ValidData::new(
            Some(Arc::new(data)),
            wrapped_msg.qbft_message.root,
        ));
        Ok((valid_data, *signer))
    }

    // ------------------------------
    // HANDLE RECEIVING A NEW MESSAGE
    // ------------------------------

    /// Receive a new message from the network
    pub fn receive(&mut self, wrapped_msg: WrappedQbftMessage) -> Result<(), QbftError> {
        // Perform base qbft releveant verification on the message
        let (valid_data, signer) = match self.validate_message(&wrapped_msg) {
            Ok((Some(data), signer)) => (data, signer),
            Ok((None, _)) => return Ok(()),
            Err(e) => return Err(e),
        };

        let msg_round: Round = wrapped_msg.qbft_message.round.into();

        // All basic verification successful! Dispatch to the correct handler
        match wrapped_msg.qbft_message.qbft_message_type {
            QbftMessageType::Proposal => {
                self.received_propose(valid_data, signer, msg_round, wrapped_msg)?
            }
            QbftMessageType::Prepare => self.received_prepare(signer, msg_round, wrapped_msg)?,
            QbftMessageType::Commit => {
                if wrapped_msg.signed_message.operator_ids().len() == 1 {
                    self.received_commit(signer, msg_round, wrapped_msg)?
                } else {
                    self.received_decided(wrapped_msg)?
                }
            }
            QbftMessageType::RoundChange => {
                self.received_round_change(signer, msg_round, wrapped_msg)?
            }
        }

        Ok(())
    }

    // We have received a new Proposal messaage
    fn received_propose(
        &mut self,
        valid_data: ValidData<D>,
        operator_id: OperatorId,
        round: Round,
        wrapped_msg: WrappedQbftMessage,
    ) -> Result<(), QbftError> {
        // Make sure that we are actually waiting for a proposal (for current round)
        // Allow future round proposals even in other states
        if round == self.current_round && !matches!(self.state, InstanceState::AwaitingProposal) {
            debug!(from=?operator_id, ?self.state, "PROPOSE message for current round while in invalid state");
            return Err(QbftError::InvalidState);
        }

        // Make sure the hash matches the data
        let mut hasher = Sha256::new();
        hasher.update(wrapped_msg.signed_message.full_data());
        let hash_bytes: [u8; 32] = hasher.finalize().into();
        let computed_hash = Hash256::from(hash_bytes);

        if computed_hash != wrapped_msg.qbft_message.root {
            return Err(QbftError::InvalidFullData);
        }

        // Make sure this is from the leader
        if !self.check_leader(&operator_id) {
            return Err(QbftError::ProposalNotFromLeader);
        }

        // If we are passed the first round, make sure that the justifications actually justify the
        // received proposal
        if round > Round::default() {
            // validate the justifications
            self.validate_justifications(&wrapped_msg)?
        };

        // Fulldata is included in propose messages
        let data = match valid_data.data {
            Some(data) => data,
            None => {
                warn!(from = ?operator_id, "Proposal should contain data");
                return Err(QbftError::ProposalMissingData);
            }
        };
        self.data.insert(valid_data.hash, data);

        debug!(from = ?operator_id, state = ?self.state, "PROPOSE received");

        // Store the received propse message
        if !self
            .propose_container
            .add_message(round, operator_id, &wrapped_msg)
        {
            warn!(from = ?operator_id, "PROPOSE message is a duplicate");
            return Err(QbftError::DuplicateProposal);
        }

        // Make sure we have not already accepted another proposal for this round.
        // Only reject if we've already accepted a proposal for THIS round
        // Allow proposals for future rounds even if we have a proposal for current round
        // question the second part
        if self.proposal_accepted_for_current_round && round == self.current_round {
            warn!(from = ?operator_id, "Proposal has already been accepted for this round");
            return Err(QbftError::ProposalAlreadyReceived);
        }

        // If this is a future round proposal, update our round to match
        // This matches Go's behavior in uponProposal at line 28
        if round > self.current_round {
            debug!(old_round = ?self.current_round, new_round = ?round, "Updating to future round from proposal");
            self.current_round = round;
            // todo we need to send round change here
        }

        // Accept this proposal
        self.proposal_accepted_for_current_round = true;
        self.proposal_root = Some(valid_data.hash);
        self.state = InstanceState::Prepare {
            proposal_root: valid_data.hash,
        };
        debug!(state = ?self.state, "State updated to PREPARE");

        // Create and send prepare message
        self.send_prepare(wrapped_msg.qbft_message.root);

        Ok(())
    }

    /// We have received a prepare message
    fn received_prepare(
        &mut self,
        operator_id: OperatorId,
        round: Round,
        wrapped_msg: WrappedQbftMessage,
    ) -> Result<(), QbftError> {
        // If we are already done, ignore (matches received_commit behavior)
        if self.completed.is_some() {
            return Ok(());
        }

        // Check that we are in the correct state. We do not have to be in the PREPARE state right
        // now as this message may have been delayed
        if u8::from(self.state) >= u8::from(InstanceState::SentRoundChange) {
            debug!(from=?operator_id, ?self.state, "PREPARE message while in invalid state");
            return Err(QbftError::InvalidState);
        }

        // Make sure this is actually a prepare message
        if !(matches!(
            wrapped_msg.qbft_message.qbft_message_type,
            QbftMessageType::Prepare,
        )) {
            warn!(from=?operator_id, "Expected a PREPARE message");
            return Err(QbftError::WrongMessageType);
        }

        debug!(from = ?operator_id, state = ?self.state, "PREPARE received");

        // Make sure that we have accepted a proposal for this round
        if !self.proposal_accepted_for_current_round {
            debug!(from=?operator_id, ?self.state, "Have not accepted Proposal for current round yet");
            return Err(QbftError::NoProposalAccepted);
        }

        // Check that the prepare message is for the accepted proposal
        if let Some(accepted_root) = self.proposal_root
            && wrapped_msg.qbft_message.root != accepted_root
        {
            warn!(from=?operator_id, "PREPARE message for different root than accepted proposal");
            return Err(QbftError::ProposedDataMismatch);
        }

        // Store the prepare message
        if !self
            .prepare_container
            .add_message(round, operator_id, &wrapped_msg)
        {
            warn!(from = ?operator_id, "PREPARE message is a duplicate")
        }

        // Check if we have reached a prepare quorum for this round, if so send the commit message
        if let Some(hash) = self.prepare_container.has_quorum(round) {
            // Make sure we are in the correct state or have already moved past it
            let proposal_root = match self.state {
                InstanceState::Prepare { proposal_root } => proposal_root,
                _ => {
                    debug!(from=?operator_id, ?self.state, "Not in PREPARE state");
                    return Ok(());
                }
            };

            // Make sure that the root of the data that we have come to a prepare consensus on
            // matches the root of the proposal that we have accepted
            if hash != proposal_root {
                warn!("PREPARE quorum root does not match accepted PROPOSAL root");
                return Err(QbftError::ProposedDataMismatch);
            }

            // Success! We have come to a prepare consensus on a value

            // Move the state forward since we have a prepare quorum (only if not already in Commit
            // state)
            self.state = InstanceState::Commit { proposal_root };

            // Record that we have come to a consensus on this value
            self.past_consensus.insert(round, hash);

            // Record as last prepared value and round
            self.last_prepared_value = Some(hash);
            self.last_prepared_round = Some(self.current_round);

            // Send a commit message for the prepare quorum data (only if we just transitioned)
            self.send_commit(hash);
        }

        Ok(())
    }

    /// We have received a commit message
    fn received_commit(
        &mut self,
        operator_id: OperatorId,
        round: Round,
        wrapped_msg: WrappedQbftMessage,
    ) -> Result<(), QbftError> {
        // If we are already done, ignore
        if self.completed.is_some() {
            return Ok(());
        }

        // Make sure that we are in the correct state
        if u8::from(self.state) >= u8::from(InstanceState::SentRoundChange) {
            return Err(QbftError::InvalidState);
        }

        // Make sure this is actually a commit message
        if !(matches!(
            wrapped_msg.qbft_message.qbft_message_type,
            QbftMessageType::Commit,
        )) {
            warn!(from=?operator_id, "Expected a COMMIT message");
            return Err(QbftError::WrongMessageType);
        }

        // If we have accepted a proposal, check that the commit matches it
        // But allow commits without proposal (for catch-up scenarios)
        if self.proposal_accepted_for_current_round {
            if let Some(accepted_root) = self.proposal_root {
                if wrapped_msg.qbft_message.root != accepted_root {
                    return Err(QbftError::ProposedDataMismatch);
                }
            }
        } else {
            debug!(from=?operator_id, ?self.state, "Have not accepted Proposal for current round yet");
            return Err(QbftError::NoProposalAccepted);
        };

        debug!(from = ?operator_id, state = ?self.state, "COMMIT received");

        // Store the received commit message
        if !self
            .commit_container
            .add_message(round, operator_id, &wrapped_msg)
        {
            warn!(from = ?operator_id, "COMMIT message is a duplicate")
        }

        // Check if we have a commit quorum
        if let Some(hash) = self.commit_container.has_quorum(round) {
            debug!(
                "Commit quorum detected for round {} with hash {:?}",
                round, hash
            );
            // Handle commit quorum based on our current state
            match self.state {
                InstanceState::Commit { proposal_root } => {
                    // We already accepted a proposal and are in commit state
                    if hash != proposal_root {
                        warn!("COMMIT quorum root does not match accepted PROPOSAL root");
                        return Err(QbftError::ProposedDataMismatch);
                    }
                }
                InstanceState::Prepare { proposal_root } => {
                    // Transition to Commit state first
                    if hash != proposal_root {
                        warn!("COMMIT quorum root does not match accepted PROPOSAL root");
                        return Err(QbftError::ProposedDataMismatch);
                    }
                    self.state = InstanceState::Commit { proposal_root };
                }
                _ => {
                    return Err(QbftError::InvalidState);
                }
            }

            // Aggregate all of the commit messages
            let commit_quorum = self.commit_container.get_quorum_of_messages(round);
            let aggregated_commit = self.aggregate_commit_messages(commit_quorum);
            if aggregated_commit.is_some() {
                debug!(state = ?self.state, "Reached a COMMIT consensus. Success!");
                self.aggregated_commit = aggregated_commit;
                self.state = InstanceState::Complete;
                self.completed = Some(Completed::Success(hash));
            } else {
                error!("Failed to aggregate commit quorum")
            }
        }

        Ok(())
    }

    /// We have received a round change message.
    fn received_round_change(
        &mut self,
        operator_id: OperatorId,
        round: Round,
        wrapped_msg: WrappedQbftMessage,
    ) -> Result<(), QbftError> {
        // Make sure we are in the correct state
        if u8::from(self.state) >= u8::from(InstanceState::Complete) {
            debug!(from=*operator_id, ?self.state, "ROUNDCHANGE message while in invalid state");
            return Ok(());
        }

        debug!(from = ?operator_id, state = ?self.state, "ROUNDCHANGE received");

        // Step 1: hasQuorumBefore check - exactly like Go's uponRoundChange
        // Get messages for this specific round BEFORE adding the new message
        let messages_for_round = self.round_change_container.get_messages_for_round(round);
        let has_quorum_before = messages_for_round.len() >= self.config.quorum_size();

        // Step 2: AddFirstMsgForSignerAndRound - exactly like Go
        let added_msg = self
            .round_change_container
            .add_message(round, operator_id, &wrapped_msg);

        if !added_msg {
            // Message was already added from signer - return like Go does
            return Ok(());
        }

        // Step 3: Early exit if we already had quorum - exactly like Go
        if has_quorum_before {
            return Ok(()); // already changed round
        }

        // Validate RoundChange justifications if present
        if !wrapped_msg
            .qbft_message
            .round_change_justification
            .is_empty()
        {
            // RoundChange messages can include prepare justifications
            // These prove what value was previously prepared
            self.validate_round_change_justifications(&wrapped_msg)?;
        }

        // There are two cases to check here

        // 1. If we have received a quorum of round change messages, we need to start a new round
        if self.round_change_container.has_quorum(round).is_some() {
            // If we're the leader for the target round, we can proceed directly to the new round
            // even if we haven't sent a round change ourselves
            let is_leader = self.check_leader(&self.config.operator_id());

            if matches!(self.state, InstanceState::SentRoundChange) || is_leader {
                // Don't process if we're already at the target round and have moved past initial
                // state This prevents duplicate proposals when RC quorum is reached
                // multiple times
                if self.current_round == round
                    && !matches!(self.state, InstanceState::SentRoundChange)
                {
                    // We're at the target round. Only proceed if:
                    // 1. We're in AwaitingProposal and haven't accepted a proposal yet (first time)
                    // 2. We're in SentRoundChange (waiting for quorum)
                    // Otherwise, we've already processed RC quorum for this round
                    if !(matches!(self.state, InstanceState::AwaitingProposal)
                        && !self.proposal_accepted_for_current_round)
                    {
                        return Ok(());
                    }
                }

                // If we have reached a quorum for this round and have already sent a round change,
                // OR if we're the leader, advance to that round.
                debug!(round = *round, "Round change quorum reached");

                // We have reached consensus on a round change, we can start a new round now
                self.state = InstanceState::RoundChangeConsensus;

                // The round change messages is round + 1, so this is the next round we want to use
                self.set_round(round);
            }
        } else {
            let round = self
                .round_change_container
                .highest_partial_quorum_above_round(self.current_round, self.config.get_f() + 1);

            if let Some(new_round) = round {
                if new_round > self.current_round {
                    // Update round - exactly like Go
                    self.current_round = new_round;

                    // Clear proposal accepted state - exactly like Go
                    self.proposal_accepted_for_current_round = false;

                    // Set state to SentRoundChange - exactly like Go
                    self.state = InstanceState::SentRoundChange;

                    // Send round change message - exactly like Go
                    let data_hash = self.last_prepared_value.unwrap_or_default();
                    self.send_round_change(data_hash);
                    // Note: We don't call start_round() here - this matches Go's behavior
                    // The round starts when we get full quorum, not partial quorum

                    return Ok(());
                }
            }
        }

        Ok(())
    }

    // We have received a decided message
    fn received_decided(&mut self, wrapped_msg: WrappedQbftMessage) -> Result<(), QbftError> {
        // Make sure we have a quorum of signatures
        if wrapped_msg.signed_message.operator_ids().len() < self.config().quorum_size() {
            return Ok(());
        }

        // All message and signature verification has already succeeded. Regardless of what state
        // this instance is at, we have all of the information necessary to mark it as
        // complete
        self.state = InstanceState::Complete;
        self.completed = Some(Completed::Success(wrapped_msg.qbft_message.root));
        self.aggregated_commit = Some(wrapped_msg.signed_message);

        Ok(())
    }

    // ------------------------
    // START AND END ROUNDS
    // ------------------------

    // Handles the beginning of a round.
    fn start_round(&mut self) {
        // We are waiting for consensus on a round change, do not start the round yet
        // Note: RoundChangeConsensus means we HAVE consensus and should proceed
        if matches!(self.state, InstanceState::SentRoundChange) {
            return;
        }

        debug!(round = *self.current_round, "Starting new round");

        // Initialise the instance state for the round
        self.state = InstanceState::AwaitingProposal;

        // Check if we are the leader
        if self.check_leader(&self.config.operator_id()) {
            // We are the leader

            // Check justification of round change quorum. If there is a justification, we will use
            // that data. Otherwise, use the initial state data
            let valid_data = self
                .justify_round_change_quorum()
                .unwrap_or_else(|| self.valid_start_data.clone());

            debug!(hash = ?valid_data.hash, "Current leader proposing data");

            // Send the initial proposal and then the following prepare
            self.send_proposal(valid_data.hash, valid_data.data.expect("Start data exists"));
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

        // Bump the current round
        self.current_round = next_round;

        // Set the state so SendRoundChange so we include Round + 1 in message
        self.state = InstanceState::SentRoundChange;

        self.send_round_change(Hash256::default());
        self.start_round();
    }

    // ---------------------
    // CREATE AND SEND NEW MESSAGES
    // ---------------------

    // Construct a new unsigned message. This will be passed to the processor to be signed and then
    // sent on the network
    fn new_unsigned_message(
        &self,
        msg_type: QbftMessageType,
        data_hash: D::Hash,
        round_change_justification: Vec<SignedSSVMessage>,
        prepare_justification: Vec<SignedSSVMessage>,
        round: Option<Round>,
    ) -> UnsignedWrappedQbftMessage {
        let data = self.get_message_data(&msg_type, data_hash);

        let round = if let Some(round) = round {
            round
        } else {
            data.round.into()
        };

        // Clear full_data from justifications as these do not store full data.
        let round_change_justification_vec: Vec<VariableList<u8, _>> = round_change_justification
            .into_iter()
            .map(|msg| msg.without_full_data())
            .map(|msg| VariableList::from(msg.as_ssz_bytes()))
            .collect();

        let prepare_justification_vec: Vec<VariableList<u8, _>> = prepare_justification
            .into_iter()
            .map(|msg| msg.without_full_data())
            .map(|msg| VariableList::from(msg.as_ssz_bytes()))
            .collect();

        let round_change_justification = VariableList::from(round_change_justification_vec);
        let prepare_justification = VariableList::from(prepare_justification_vec);

        // Create the QBFT message
        let qbft_message = QbftMessage {
            qbft_message_type: msg_type,
            height: *self.instance_height as u64,
            round: round.into(),
            identifier: (&self.identifier).into(),
            root: data.root,
            data_round: data.data_round,
            round_change_justification,
            prepare_justification,
        };

        let ssv_message = SSVMessage::new(
            MsgType::SSVConsensusMsgType,
            self.identifier.clone(),
            qbft_message.as_ssz_bytes(),
        )
        .expect("SSVMessage should be valid.");

        // Wrap in unsigned SSV message
        UnsignedWrappedQbftMessage {
            unsigned_message: UnsignedSSVMessage {
                ssv_message,
                full_data: data.full_data,
            },
            qbft_message,
        }
    }

    // Send a new qbft proposal message
    pub fn send_proposal(&mut self, hash: D::Hash, data: Arc<D>) {
        // Store the data we're proposing
        self.data.insert(hash, data.clone());

        // For Proposal messages
        // round_change_justification: rc messages proving we can move to this round
        let round_change_justifications = self.get_round_change_justifications();

        // prepare_justification: proves the value being prepared
        let (prepare_justifications, justified_value) = self.get_prepare_justifications();
        // Determine the value that should be proposed based off of justification. If we have a
        // prepare justification, we want to propose that value. Else, just propose the start data
        let value_to_propose = justified_value.unwrap_or(hash);

        // Construct a unsigned proposal
        let unsigned_msg = self.new_unsigned_message(
            QbftMessageType::Proposal,
            value_to_propose,
            round_change_justifications,
            prepare_justifications,
            None,
        );

        self.message_sender.send(unsigned_msg);
    }

    // Send a new qbft prepare message
    pub fn send_prepare(&mut self, data_hash: D::Hash) {
        // Only send prepare if we've seen this data
        if !self.data.contains_key(&data_hash) {
            warn!("Attempted to prepare unknown data");
            return;
        }

        // Construct unsigned prepare
        let unsigned_msg =
            self.new_unsigned_message(QbftMessageType::Prepare, data_hash, vec![], vec![], None);

        self.message_sender.send(unsigned_msg);
    }

    // Send a new qbft commit message
    pub fn send_commit(&mut self, data_hash: D::Hash) {
        // Construct unsigned commit
        let unsigned_msg =
            self.new_unsigned_message(QbftMessageType::Commit, data_hash, vec![], vec![], None);

        self.message_sender.send(unsigned_msg);
    }

    // Send a new qbft round change message
    pub fn send_round_change(&mut self, data_hash: D::Hash) {
        // For Round Change messages
        // round_change_justification: list of prepare messages
        let round_change_justifications = self.get_round_change_prepare_justifications();
        // prepare_justification: N/A

        // Determine the root and data round for the round change message
        // If we have a prepared value, use it. Otherwise use the passed data_hash
        let (root, data_round) = if let (Some(last_prepared_value), Some(last_prepared_round)) =
            (self.last_prepared_value, self.last_prepared_round)
        {
            // We have a prepared value, so include it in the round change
            (last_prepared_value, Some(last_prepared_round))
        } else {
            // No prepared value, use the passed data_hash with no data round
            (data_hash, None)
        };

        // Construct unsigned round change
        // Note: For RoundChange, we don't override the round - it should use current_round
        // The data_round is set automatically by new_unsigned_message based on last_prepared_round
        let unsigned_msg = self.new_unsigned_message(
            QbftMessageType::RoundChange,
            root,
            round_change_justifications,
            vec![],
            None, // Don't override the round - use current_round from MessageData
        );

        // forget that we accpeted a proposal
        self.proposal_accepted_for_current_round = false;

        self.message_sender.send(unsigned_msg);
    }

    // ----------------
    // MISC HELPERS
    // ----------------

    // Aggregate a quorum of commit messages into one signed message
    fn aggregate_commit_messages(
        &self,
        commit_quorum: Vec<WrappedQbftMessage>,
    ) -> Option<SignedSSVMessage> {
        // We know this exists, but in favor of avoiding expect match the first element to Some.
        // This will be the commit message that we aggregate on top of
        if let Some(first_commit) = commit_quorum.first() {
            let mut aggregated_commit = first_commit.signed_message.clone();
            let aggregated_ssv = aggregated_commit.ssv_message();

            // Sanity check that all of the messages match
            commit_quorum[1..]
                .iter()
                .all(|commit_msg| aggregated_ssv == commit_msg.signed_message.ssv_message())
                .then_some(())?;

            // Aggregate all of the commits together
            let signed_commits = commit_quorum[1..]
                .iter()
                .map(|msg| msg.signed_message.clone());
            aggregated_commit.aggregate(signed_commits).unwrap();

            // Set full data
            let hash = first_commit.qbft_message.root;
            aggregated_commit
                .set_full_data(self.data.get(&hash)?.as_ssz_bytes())
                .unwrap();

            return Some(aggregated_commit);
        }

        None
    }

    // Get data for the qbft message
    fn get_message_data(&self, msg_type: &QbftMessageType, data_hash: D::Hash) -> MessageData<D> {
        let full_data = if matches!(msg_type, QbftMessageType::Proposal) {
            self.data
                .get(&data_hash)
                .map(|d| d.as_ssz_bytes())
                .unwrap_or_else(|| {
                    warn!("Proposal data missing for hash {:?}", data_hash);
                    vec![]
                })
        } else {
            vec![]
        };

        // Special handling for RoundChange messages
        if matches!(msg_type, QbftMessageType::RoundChange) {
            // Check if we have a prepared value from a previous round
            if let (Some(last_prepared_value), Some(last_prepared_round)) =
                (self.last_prepared_value, self.last_prepared_round)
            {
                // We have prepare justifications - use the hash of last prepared value
                return MessageData::new(
                    last_prepared_round.get() as u64,
                    self.current_round.get() as u64,
                    last_prepared_value,
                    self.data
                        .get(&last_prepared_value)
                        .map(|d| d.as_ssz_bytes())
                        .unwrap_or_else(|| {
                            warn!("Data missing for last prepared value");
                            vec![]
                        }),
                );
            } else {
                // No prepare justifications
                return MessageData::new(
                    0, // NoRound
                    self.current_round.get() as u64,
                    Hash256::zero(), // Empty root, NOT data_hash
                    vec![],          // No full data
                );
            }
        }

        // Standard message data for Proposal (without justifications), Prepare, and Commit
        MessageData::new(0, self.current_round.get() as u64, data_hash, full_data)
    }

    /// Extract the data that the instance has come to consensus on
    pub fn completed(&self) -> Option<Completed<D>> {
        self.completed
            .clone()
            .and_then(|completed| match completed {
                // For timeout, we don't need any data
                Completed::TimedOut => Some(Completed::TimedOut),

                // For success, we need to find the actual data
                Completed::Success(hash) => {
                    // Try to get the Arc<D> from our data map
                    let data = self.data.get(&hash).cloned();

                    if data.is_none() {
                        error!("could not find finished data");
                    }

                    // Transform Arc<D> into Completed::Success(D)
                    data.map(|arc_data| Completed::Success((*arc_data).clone()))
                }
            })
    }

    // Spec test related helper functions
    // ------------------------

    /// Helper function for spec tests to set the current round
    pub fn set_current_round_spec(&mut self, round: Round) {
        self.current_round = round;
    }

    /// Helper function for spec tests to store data for proposals
    pub fn store_data_spec(&mut self, hash: D::Hash, data: D) {
        self.data.insert(hash, Arc::new(data));
    }

    /// Helper for spec tests to add messages directly to containers
    pub fn add_message_to_container_spec(&mut self, msg: &WrappedQbftMessage) {
        let round = Round::from(msg.qbft_message.round);

        for operator_id in msg.signed_message.operator_ids() {
            match msg.qbft_message.qbft_message_type {
                QbftMessageType::Proposal => {
                    self.propose_container.add_message(round, *operator_id, msg)
                }
                QbftMessageType::Prepare => {
                    self.prepare_container.add_message(round, *operator_id, msg)
                }
                QbftMessageType::Commit => {
                    self.commit_container.add_message(round, *operator_id, msg)
                }
                QbftMessageType::RoundChange => {
                    self.round_change_container
                        .add_message(round, *operator_id, msg)
                }
            };
        }
    }

    /// Helper for spec tests to check if instance is decided
    pub fn is_decided_spec(&self) -> bool {
        matches!(self.state, InstanceState::Complete)
    }

    /// Get the decided data if the instance is complete
    pub fn get_decided_data_spec(&self) -> Option<D>
    where
        D: Clone,
    {
        if matches!(self.state, InstanceState::Complete) {
            // Return the start data since that's what was decided
            // Need to dereference Arc and clone the inner value
            Some((*self.start_data).clone())
        } else {
            None
        }
    }

    /// Helper function for spec tests to set proposal accepted state
    pub fn set_proposal_accepted_spec(&mut self, root: Option<D::Hash>) {
        self.proposal_accepted_for_current_round = true;
        self.proposal_root = root;
    }

    /// Helper function for spec tests to set instance state
    pub fn set_state_spec(&mut self, state: InstanceState) {
        self.state = state;
    }

    /// Helper function for spec tests to set last prepared value and round
    pub fn set_last_prepared_spec(&mut self, value: Option<D::Hash>, round: Option<Round>) {
        self.last_prepared_value = value;
        self.last_prepared_round = round;
    }

    /// Helper function to get the commit container
    pub fn get_commit_container(&self) -> &MessageContainer {
        &self.commit_container
    }
}
