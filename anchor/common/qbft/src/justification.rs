use super::Qbft;
use std::collections::HashSet;

use crate::MessageSender;
use crate::ValidData;
use crate::error::QbftError;
use crate::qbft_types::{InstanceState, LeaderFunction, WrappedQbftMessage};
use ssv_types::{
    Round,
    consensus::{QbftData, QbftMessage, QbftMessageType},
    message::SignedSSVMessage,
};
use ssz::Decode;
use tracing::warn;
use types::Hash256;

impl<F, D, S> Qbft<F, D, S>
where
    F: LeaderFunction + Clone,
    D: QbftData<Hash = Hash256>,
    S: MessageSender,
{
    // Validate the round change and prepare justifications. Returns true if the justifications
    // correctly justify the proposal
    //
    // A QBFT Message contains fields to a list of round change justifications and prepare
    // justifications. We must go through each of these individually and verify the validity of each
    // one
    pub(crate) fn validate_round_change_justifications(
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

    pub(crate) fn validate_justifications(
        &self,
        msg: &WrappedQbftMessage,
    ) -> Result<(), QbftError> {
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
    pub(crate) fn justify_round_change_quorum(&self) -> Option<ValidData<D>> {
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

    // Get all of the round change jusitifcation messages
    pub(crate) fn get_round_change_justifications(&self) -> Vec<SignedSSVMessage> {
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
    pub(crate) fn get_prepare_justifications(&self) -> (Vec<SignedSSVMessage>, Option<Hash256>) {
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
                return (prepare_msgs, Some(prepared_value));
            }
        }

        // No prepared value found, proposer can choose new value
        (vec![], None)
    }

    /// Get justifications for a RoundChange message
    /// If we have prepared a value, include the Prepare messages that justify it
    pub(crate) fn get_round_change_prepare_justifications(&self) -> Vec<SignedSSVMessage> {
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
}
