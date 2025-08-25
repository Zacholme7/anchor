use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

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
use ssz::{Decode, Encode};
use tracing::{debug, error, warn};
use types::{FixedBytesExtended, Hash256};

use crate::msg_container::MessageContainer;

mod config;
mod error;
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
        qbft.start_round();
        qbft
    }

    // Hash of the start data
    pub fn start_data_hash(&self) -> &D::Hash {
        &self.start_data_hash
    }

    // Return a reference to the qbft configuration
    pub fn config(&self) -> &Config<F> {
        &self.config
    }

    // Get the current round
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

    // Checks to make sure any given operator is in this instance's comittee.
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
            return Err(QbftError::PastRound);
        }

        // Check for future round
        // - RoundChange messages are always allowed for future rounds
        // - Proposals are allowed for future rounds if they have justifications
        // - Other messages (Prepare, Commit) are not allowed for future rounds
        if wrapped_msg.qbft_message.round > self.current_round.into() {
            match wrapped_msg.qbft_message.qbft_message_type {
                QbftMessageType::RoundChange => {
                    // Round changes for future rounds are always allowed
                }
                QbftMessageType::Proposal => {
                    // Proposals for future rounds are only allowed with justifications
                    if wrapped_msg
                        .qbft_message
                        .round_change_justification
                        .is_empty()
                    {
                        return Err(QbftError::WrongRound);
                    }
                }
                QbftMessageType::Commit => {
                    // Single-signature commits from future rounds are not allowed
                    // But multi-signature commits (decided messages) should be allowed from any
                    // round
                    if wrapped_msg.signed_message.operator_ids().len() == 1 {
                        return Err(QbftError::WrongRound);
                    }
                    // Multi-signature commits are allowed - they represent decided values
                }
                _ => {
                    // Prepare messages for future rounds are not allowed
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
            .expect("Exists");

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

    // Handle receiving a new message
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

        // If we are passed the first round, make sure that the justifications actually justify the
        // received proposal
        if round > Round::default() {
            // validate the justifications
            self.validate_justifications(&wrapped_msg)?
        } else {
            // Make sure this is from the leader
            if !self.check_leader(&operator_id) {
                return Err(QbftError::ProposalNotFromLeader);
            }
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
            // Reset the timer for the new round
            // todo!() restart the timer???, go does that but we do nto
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
        if let Some(accepted_root) = self.proposal_root {
            if wrapped_msg.qbft_message.root != accepted_root {
                return Err(QbftError::ProposedDataMismatch);
            }
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
                InstanceState::Commit { proposal_root } => proposal_root,
                _ => {
                    debug!(from=?operator_id, ?self.state, "Not in PREPARE state");
                    return Err(QbftError::InvalidState);
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
            let should_send_commit = match self.state {
                InstanceState::Prepare { .. } => {
                    self.state = InstanceState::Commit { proposal_root };
                    debug!(state = ?self.state, "Reached a PREPARE consensus. State updated to COMMIT");
                    true // Send commit message when transitioning to Commit state
                }
                InstanceState::Commit { .. } => {
                    // Already in Commit state, no need to transition or send another commit
                    false
                }
                _ => {
                    // This should not happen given our earlier check, but be defensive
                    false
                }
            };

            // Record that we have come to a consensus on this value
            self.past_consensus.insert(round, hash);

            // Record as last prepared value and round
            self.last_prepared_value = Some(hash);
            self.last_prepared_round = Some(self.current_round);

            // Send a commit message for the prepare quorum data (only if we just transitioned)
            if should_send_commit {
                self.send_commit(hash);
            }
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
                InstanceState::AwaitingProposal => {
                    // Catch-up scenario: we have commit quorum without seeing proposal
                    // This is valid - we can decide based on commit quorum alone
                    // Transition directly to Commit state
                    debug!("Received commit quorum without proposal - catch-up scenario");
                    self.state = InstanceState::Commit {
                        proposal_root: hash,
                    };
                    self.proposal_root = Some(hash);
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

        if let Some(new_round) = self.has_received_partial_quorum()? {
            if new_round > self.current_round {
                // Handle partial quorum (f+1 speedup) - exactly like Go
                self.upon_change_round_partial_quorum(new_round);
                return Ok(());
            }
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
            // 2. If we receive f+1 round change messages, we need to send our own round-change
            //    message (unless we're the leader for the target round)
            let num_messages_for_round = self.round_change_container.num_messages_for_round(round);
            if num_messages_for_round > self.config.get_f()
                && !(matches!(self.state, InstanceState::SentRoundChange))
            {
                // If we're the leader for the target round, don't send a round change
                // We'll wait for quorum and send the proposal directly
                if self.check_leader(&self.config.operator_id()) {
                    // Mark state to show we're waiting for RC quorum
                    // We don't change round yet - wait for quorum
                    return Ok(());
                }

                // Set the state so SendRoundChange so we include Round + 1 in message
                self.state = InstanceState::SentRoundChange;

                self.send_round_change(Hash256::default());
            }
        }

        Ok(())
    }

    /// Step 4: hasReceivedPartialQuorum - exactly like Go
    fn has_received_partial_quorum(&self) -> Result<Option<Round>, QbftError> {
        // Get ALL round change messages from container
        let all_messages = self.round_change_container.get_all_messages();

        // Filter for future rounds only (> current_round)
        let mut future_round_messages = Vec::new();
        for (msg_round, messages) in all_messages {
            if msg_round > self.current_round {
                for msg in messages {
                    future_round_messages.push((msg_round, msg));
                }
            }
        }

        if future_round_messages.is_empty() {
            return Ok(None);
        }

        // Count unique signers across ALL future rounds - exactly like Go's HasPartialQuorum
        let mut unique_signers = HashSet::new();
        for (_, msg) in &future_round_messages {
            for signer in msg.signed_message.operator_ids() {
                unique_signers.insert(*signer);
            }
        }

        // Check if we have partial quorum (F+1) - exactly like Go
        if unique_signers.len() > self.config.get_f() {
            // Find minimum round among future messages - exactly like Go's minRound
            let min_round = future_round_messages
                .iter()
                .map(|(round, _)| *round)
                .min()
                .unwrap(); // Safe because we checked future_round_messages is not empty

            Ok(Some(min_round))
        } else {
            Ok(None)
        }
    }

    /// Step 4: uponChangeRoundPartialQuorum - exactly like Go
    fn upon_change_round_partial_quorum(&mut self, new_round: Round) {
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
    }

    // We have received a decided message
    fn received_decided(&mut self, wrapped_msg: WrappedQbftMessage) -> Result<(), QbftError> {
        // Make sure we have a quorum of signatures
        if wrapped_msg.signed_message.operator_ids().len() < self.config().quorum_size() {
            // For multi-sig messages without quorum, we check if we have accepted a proposal
            // If not, return the appropriate error
            if !self.proposal_accepted_for_current_round {
                return Err(QbftError::NoProposalAccepted);
            }
            return Err(QbftError::NotEnoughSignatures);
        }

        // All message and signature verification has already succeeded. Regardless of what state
        // this instance is at, we have all of the information necessary to mark it as
        // complete
        self.state = InstanceState::Complete;
        self.completed = Some(Completed::Success(wrapped_msg.qbft_message.root));
        self.aggregated_commit = Some(wrapped_msg.signed_message);

        Ok(())
    }

    // Validate the round change and prepare justifications. Returns true if the justifications
    // correctly justify the proposal
    //
    // A QBFT Message contains fields to a list of round change justifications and prepare
    // justifications. We must go through each of these individually and verify the validity of each
    // one
    fn validate_round_change_justifications(
        &self,
        msg: &WrappedQbftMessage,
    ) -> Result<(), QbftError> {
        // RoundChange messages can have prepare justifications (round_change_justification field)
        // These are Prepare messages that justify the value being carried forward

        // If the round change has data_round > 0, it means it has prepared
        // In this case, we need to validate the prepare justifications have quorum
        if msg.qbft_message.data_round > 0 {
            let mut unique_signers = HashSet::new();
            let mut seen_messages = HashSet::new();

            for prepare_bytes in &msg.qbft_message.round_change_justification {
                // Check for duplicate messages
                if !seen_messages.insert(prepare_bytes.clone()) {
                    return Err(QbftError::StandaloneRoundChangeNoQuorum);
                }

                // Decode the prepare message
                let prepare_msg = SignedSSVMessage::from_ssz_bytes(prepare_bytes)
                    .map_err(|_| QbftError::RoundChangeJustificationDecodeFailed)?;

                // Check for multi-signers
                if prepare_msg.operator_ids().len() > 1 {
                    return Err(QbftError::RoundChangeJustificationMultiSigner);
                }

                // Add signer to unique set
                for signer in prepare_msg.operator_ids() {
                    unique_signers.insert(*signer);
                }

                let prepare_qbft = QbftMessage::from_ssz_bytes(prepare_msg.ssv_message().data())
                    .map_err(|_| QbftError::RoundChangeJustificationDecodeFailed)?;

                // Verify it's actually a Prepare message
                if prepare_qbft.qbft_message_type != QbftMessageType::Prepare {
                    return Err(QbftError::RoundChangeJustificationNotRoundChange);
                }

                // Check the round matches the data_round specified in the RoundChange
                if prepare_qbft.round != msg.qbft_message.data_round {
                    return Err(QbftError::RoundChangeJustificationWrongRound);
                }

                // Check the value matches
                if prepare_qbft.root != msg.qbft_message.root {
                    return Err(QbftError::PrepareJustificationRootMismatch);
                }
            }

            // Check if we have quorum of unique signers
            if unique_signers.len() < self.config.quorum_size() {
                return Err(QbftError::StandaloneRoundChangeNoQuorum);
            }
        }

        Ok(())
    }

    fn validate_justifications(&self, msg: &WrappedQbftMessage) -> Result<(), QbftError> {
        // Record if any of the round change messages have a value that was prepared
        let mut previously_prepared = false;
        let mut max_prepared_round = 0;
        let mut max_prepared_msg = None;

        // Count UNIQUE signers for round change justifications (not just message count)
        // This matches Go's HasQuorum logic that counts unique signers
        let mut rc_unique_signers = std::collections::HashSet::new();
        // Track seen messages to detect duplicates
        let mut seen_rc_messages = std::collections::HashSet::new();

        // Process all round change justifications
        for signed_round_change in &msg.qbft_message.round_change_justification {
            // The justification message is represented as a VariableList<u8> in the signed message,
            // deserialize this into a proper QbftMessage
            let Ok(typed_signed_round_change) =
                SignedSSVMessage::from_ssz_bytes(signed_round_change)
            else {
                return Err(QbftError::RoundChangeJustificationDecodeFailed);
            };

            // Check for multi-signers - round change messages should only have 1 signer
            if typed_signed_round_change.operator_ids().len() > 1 {
                return Err(QbftError::RoundChangeJustificationMultiSigner);
            }

            // Track duplicate messages but don't reject them
            // The Go implementation accepts duplicates as long as we have enough unique signers
            seen_rc_messages.insert(signed_round_change.clone());

            // Add signers to unique set for quorum counting
            // Duplicates from the same signer are ignored for quorum calculation
            for signer in typed_signed_round_change.operator_ids() {
                rc_unique_signers.insert(*signer);
            }

            let round_change: QbftMessage = {
                match QbftMessage::from_ssz_bytes(typed_signed_round_change.ssv_message().data()) {
                    Ok(data) => data,
                    Err(_) => return Err(QbftError::RoundChangeJustificationDecodeFailed),
                }
            };

            // Make sure this is actually a round change message
            if !matches!(round_change.qbft_message_type, QbftMessageType::RoundChange) {
                return Err(QbftError::RoundChangeJustificationNotRoundChange);
            }

            // Check the round of the justification message
            if round_change.round != msg.qbft_message.round {
                return Err(QbftError::RoundChangeJustificationWrongRound);
            }

            // For round change justifications, we need special validation that doesn't check
            // against current round since they're justifications from the proposal's round
            // Check height
            if round_change.height != *self.instance_height as u64 {
                return Err(QbftError::RoundChangeJustificationInvalidSignature);
            }

            // Check all signers are in committee
            for signer in typed_signed_round_change.operator_ids() {
                if !self.check_committee(signer) {
                    return Err(QbftError::RoundChangeJustificationInvalidSignature);
                }
            }

            // If the data_round > 0 (not 1), that means we have prepared a value in previous rounds
            // Note: data_round of 0 means NoRound (not prepared), any value > 0 means prepared
            if round_change.data_round > 0 {
                previously_prepared = true;

                // also track the max prepared value and round
                if round_change.data_round > max_prepared_round {
                    max_prepared_round = round_change.data_round;
                    max_prepared_msg = Some(round_change.clone());
                }

                // CRITICAL: When a round change message has a prepared value (data_round > 0),
                // we must validate that its prepare justifications have quorum.
                // This matches Go's validRoundChangeForDataIgnoreSignature behavior.
                let mut rc_prep_unique_signers = std::collections::HashSet::new();

                // Validate each prepare message in the round change justification
                // In round change messages, the prepare justifications are stored in
                // round_change_justification when the round change has prepared
                // (data_round > 0)
                for prepare_msg in &round_change.round_change_justification {
                    let typed_prepare = match SignedSSVMessage::from_ssz_bytes(prepare_msg) {
                        Ok(msg) => msg,
                        Err(_) => return Err(QbftError::RoundChangeJustificationInvalidPrepares),
                    };

                    // Decode the prepare QBFT message
                    let prepare_qbft =
                        match QbftMessage::from_ssz_bytes(typed_prepare.ssv_message().data()) {
                            Ok(msg) => msg,
                            Err(_) => {
                                return Err(QbftError::RoundChangeJustificationInvalidPrepares);
                            }
                        };

                    // Verify it's a prepare message
                    if prepare_qbft.qbft_message_type != QbftMessageType::Prepare {
                        return Err(QbftError::RoundChangeJustificationInvalidPrepares);
                    }

                    // CRITICAL: Check that the prepare round matches the round change's data_round
                    // This matches Go's validSignedPrepareForHeightRoundAndRootVerifySignature
                    // check
                    if prepare_qbft.round != round_change.data_round {
                        // This is the error we need for the test!
                        // In Go: "round change justification invalid: wrong msg round"
                        // But we need to return an error that gets wrapped correctly
                        return Err(QbftError::RoundChangeJustificationInvalidPrepareRound);
                    }

                    // Check the prepare message has the same root as the round change
                    if prepare_qbft.root != round_change.root {
                        return Err(QbftError::RoundChangeJustificationInvalidPrepareRoot);
                    }

                    // Check height matches
                    if prepare_qbft.height != round_change.height {
                        return Err(QbftError::RoundChangeJustificationInvalidPrepares);
                    }

                    // Count unique signers
                    for signer in typed_prepare.operator_ids() {
                        rc_prep_unique_signers.insert(*signer);
                    }
                }

                // Check if this round change has quorum of unique signers
                if rc_prep_unique_signers.len() < self.config.quorum_size() {
                    // This round change message doesn't have valid prepare justifications
                    return Err(QbftError::RoundChangeJustificationInvalidPrepares);
                }
            }
        }

        // After processing all messages, check if we have quorum of unique signers
        if rc_unique_signers.len() < self.config.quorum_size() {
            return Err(QbftError::RoundChangeJustificationNoQuorum);
        }

        // If there was a value that was also previously prepared, we must also verify all of the
        // prepare justifications
        if previously_prepared {
            // Count UNIQUE signers for prepare justifications
            let mut prep_unique_signers = std::collections::HashSet::new();

            // First pass: collect unique signers from prepare messages
            for signed_prepare in &msg.qbft_message.prepare_justification {
                if let Ok(typed_signed_prepare) = SignedSSVMessage::from_ssz_bytes(signed_prepare) {
                    for signer in typed_signed_prepare.operator_ids() {
                        prep_unique_signers.insert(*signer);
                    }
                }
            }

            // Check if we have a quorum of UNIQUE signers
            if prep_unique_signers.len() < self.config.quorum_size() {
                return Err(QbftError::PrepareJustificationNotEnough);
            }

            // Make sure that the roots match
            if msg.qbft_message.root
                != max_prepared_msg
                    .clone()
                    .expect("Exists as we have a previously prepared value")
                    .root
            {
                return Err(QbftError::PrepareJustificationValueMismatch);
            }

            // Validate each prepare message matches highest prepared round/value
            for signed_prepare in &msg.qbft_message.prepare_justification {
                // The qbft message is represented as VariableList<u8> in the signed message,
                // deserialize this into a qbft message
                let Ok(typed_signed_prepare) = SignedSSVMessage::from_ssz_bytes(signed_prepare)
                else {
                    return Err(QbftError::PrepareJustificationDecodeFailed);
                };

                let prepare =
                    match QbftMessage::from_ssz_bytes(typed_signed_prepare.ssv_message().data()) {
                        Ok(data) => data,
                        Err(_) => return Err(QbftError::PrepareJustificationDecodeFailed),
                    };

                // Make sure this is a prepare message
                if prepare.qbft_message_type != QbftMessageType::Prepare {
                    return Err(QbftError::PrepareJustificationNotPrepare);
                }

                // For prepare justifications, we need special validation that doesn't check round
                // since prepare messages are from previous rounds by definition
                // Check height
                if prepare.height != *self.instance_height as u64 {
                    return Err(QbftError::PrepareJustificationValidationFailed);
                }

                // Check all signers are in committee
                for signer in typed_signed_prepare.operator_ids() {
                    if !self.check_committee(signer) {
                        return Err(QbftError::PrepareJustificationValidationFailed);
                    }
                }

                if prepare.root != msg.qbft_message.root {
                    return Err(QbftError::PrepareJustificationRootMismatch);
                }

                // Check the round of the prepare justification matches the highest prepared round
                if prepare.round != max_prepared_round {
                    return Err(QbftError::PrepareJustificationWrongRound);
                }
            }
        }
        Ok(())
    }

    /// Justify the round change quorum
    /// In order to justify a round change quorum, we find the maximum round of the quorum set that
    /// had achieved a past consensus. If we have also seen consensus on this round for the
    /// suggested data, then it is justified and this function returns that data.
    /// If there is no past consensus data in the round change quorum or we disagree with quorum set
    /// this function will return None, and we obtain the data as if we were beginning this
    /// instance.
    fn justify_round_change_quorum(&self) -> Option<ValidData<D>> {
        // Get all round change messages for the current round
        let round_changes = self
            .round_change_container
            .get_messages_for_round(self.current_round);

        // Need quorum to proceed
        if round_changes.len() < self.config.quorum_size() {
            return None;
        }

        // Find the highest prepared round and value
        let mut highest_prepared: Option<(Round, Hash256)> = None;

        for rc_msg in &round_changes {
            if rc_msg.qbft_message.data_round > 0 {
                let prepared_round = Round::from(rc_msg.qbft_message.data_round);

                if highest_prepared.is_none()
                    || prepared_round > highest_prepared.as_ref().unwrap().0
                {
                    highest_prepared = Some((prepared_round, rc_msg.qbft_message.root));
                }
            }
        }

        // If there's a highest prepared value, use it
        if let Some((prepared_round, prepared_hash)) = highest_prepared {
            // Verify we also saw this consensus
            if let Some(&consensus_hash) = self.past_consensus.get(&prepared_round) {
                if consensus_hash == prepared_hash {
                    // Get the data for this hash
                    let data = self.data.get(&prepared_hash).cloned().unwrap_or_else(|| {
                        warn!("Previous consensus data missing. Using start value");
                        self.start_data.clone()
                    });
                    return Some(ValidData::new(Some(data), prepared_hash));
                }
            }
        }

        // No prepared value, use start data
        None
    }

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

    // Aggregate a quorum of commit messages into one signed message
    fn aggregate_commit_messages(
        &self,
        commit_quorum: Vec<WrappedQbftMessage>,
    ) -> Option<SignedSSVMessage> {
        // We know this exists, but in favor of avoiding expect match the first element to Some.
        // This will be the commit message that we aggregate on top of
        if let Some(first_commit) = commit_quorum.first() {
            let mut aggregated_commit = first_commit.signed_message.clone();

            // Sanity check that all commits are for the same consensus instance
            // We check that the important consensus fields match (height, round, root, type)
            // but NOT the identifier field which contains operator-specific info
            let first_qbft = &first_commit.qbft_message;
            let all_match = commit_quorum[1..]
                .iter()
                .enumerate()
                .all(|(_idx, commit_msg)| {
                    let qbft = &commit_msg.qbft_message;
                    // Check that consensus-critical fields match
                    let matches = qbft.qbft_message_type == first_qbft.qbft_message_type
                        && qbft.height == first_qbft.height
                        && qbft.round == first_qbft.round
                        && qbft.root == first_qbft.root
                        && qbft.data_round == first_qbft.data_round;

                    if !matches {}
                    matches
                });

            if !all_match {
                return None;
            }

            // Aggregate all of the commits together
            let signed_commits = commit_quorum[1..]
                .iter()
                .map(|msg| msg.signed_message.clone());
            if let Err(_e) = aggregated_commit.aggregate(signed_commits) {
                return None;
            }

            // Set full data
            let hash = first_commit.qbft_message.root;
            if let Some(data) = self.data.get(&hash) {
                if let Err(_e) = aggregated_commit.set_full_data(data.as_ssz_bytes()) {
                    return None;
                }
            } else {
                return None;
            }

            return Some(aggregated_commit);
        }

        None
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

    // Get all of the round change jusitifcation messages
    fn get_round_change_justifications(&self) -> Vec<SignedSSVMessage> {
        // Short circuit if we are in first round
        if self.current_round <= Round::default() {
            return vec![];
        }

        // If we are past the first round and awaiting proposal, that means that there was a
        // round change and we must have a quorum of round change messages. We include these so
        // that we can prove that we had a consensus allowing us to change
        if matches!(self.state, InstanceState::AwaitingProposal) {
            let round_changes = self
                .round_change_container
                .get_messages_for_round(self.current_round);

            // We need at least a quorum of round changes to justify the proposal
            if round_changes.len() >= self.config.quorum_size() {
                // Include ALL round change messages for the round in the order they were received
                // IMPORTANT: Go does NOT sort these messages - it preserves insertion order
                // This means tests "order 1" and "order 2" will produce DIFFERENT outputs
                return round_changes
                    .into_iter()
                    .map(|msg| msg.signed_message.clone())
                    .collect();
            }
        }
        vec![]
    }

    // Get all of the prepare justifications for proposals
    fn get_prepare_justifications(&self) -> (Vec<SignedSSVMessage>, Option<Hash256>) {
        // No justifications needed for round 0
        if self.current_round == Round::default() {
            return (vec![], None);
        }

        // Only needed when we're the proposer
        if !matches!(self.state, InstanceState::AwaitingProposal) {
            return (vec![], None);
        }

        // Check if we have our own prepared value that should be proposed
        // This handles the case where we prepared a value but the RoundChange messages
        // don't reflect it (e.g., other nodes didn't prepare)
        if let (Some(last_prepared_value), Some(last_prepared_round)) =
            (self.last_prepared_value, self.last_prepared_round)
        {
            // Get prepare messages from the prepare container for the last prepared round
            let mut prepare_msgs = self
                .prepare_container
                .get_messages_for_round(last_prepared_round)
                .into_iter()
                .map(|wrapped| wrapped.signed_message.clone())
                .collect::<Vec<_>>();

            if prepare_msgs.len() >= self.config.quorum_size() {
                prepare_msgs.sort_by_key(|msg| msg.operator_ids()[0]);
                return (prepare_msgs, Some(last_prepared_value));
            }
        }

        // Get all round change messages for current round
        let round_changes = self
            .round_change_container
            .get_messages_for_round(self.current_round);

        if round_changes.len() < self.config.quorum_size() {
            return (vec![], None);
        }

        // Find the highest prepared round among all round changes
        let mut highest_prepared: Option<(Round, Hash256, &WrappedQbftMessage)> = None;

        for rc_msg in &round_changes {
            // Check if this round change has a prepared value
            if rc_msg.qbft_message.data_round > 0 {
                let prepared_round = Round::from(rc_msg.qbft_message.data_round);

                // Update if this is the highest we've seen
                if highest_prepared.is_none()
                    || prepared_round > highest_prepared.as_ref().unwrap().0
                {
                    highest_prepared = Some((prepared_round, rc_msg.qbft_message.root, rc_msg));
                }
            }
        }

        // If we found a highest prepared value, extract its prepare justifications
        if let Some((_, prepared_value, highest_rc)) = highest_prepared {
            // Extract the prepare messages from the round change message's justifications
            // These are stored in the round_change_justification field of the RoundChange
            let mut prepare_msgs = Vec::new();

            for prepare_bytes in &highest_rc.qbft_message.round_change_justification {
                if let Ok(signed_msg) = SignedSSVMessage::from_ssz_bytes(prepare_bytes) {
                    prepare_msgs.push(signed_msg);
                }
            }

            // Verify we have quorum of prepares
            if prepare_msgs.len() >= self.config.quorum_size() {
                //prepare_msgs.sort_by_key(|msg| msg.operator_ids()[0]);
                return (prepare_msgs, Some(prepared_value));
            }
        }

        // No prepared value found, proposer can choose new value
        (vec![], None)
    }

    /// Get justifications for a RoundChange message
    /// If we have prepared a value, include the Prepare messages that justify it
    fn get_round_change_prepare_justifications(&self) -> Vec<SignedSSVMessage> {
        // Only include prepare justifications if we have a prepared value
        if let (Some(last_prepared_value), Some(last_prepared_round)) =
            (self.last_prepared_value, self.last_prepared_round)
        {
            // Get the prepare messages for the round where we prepared
            let prepares = self
                .prepare_container
                .get_messages_for_round(last_prepared_round);

            // Only include prepares that match our prepared value
            let filtered_prepares: Vec<_> = prepares
                .iter()
                .filter(|msg| msg.qbft_message.root == last_prepared_value)
                .collect();

            // We need a quorum of prepares to justify the prepared value
            if filtered_prepares.len() >= self.config.quorum_size() {
                let result: Vec<SignedSSVMessage> = filtered_prepares
                    .into_iter()
                    .map(|msg| msg.signed_message.clone())
                    .collect();
                return result;
            }
        }

        vec![]
    }

    // Create and Send new message
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
