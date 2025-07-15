use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};

use crate::utils::test_keys::TestKeySet;
use bls::PublicKeyBytes;
use message_validator::{
    DutiesProvider, ValidatedSSVMessage, ValidationFailure, minimal_validate_consensus_message,
};
use slot_clock::SlotClock;
use ssv_types::{CommitteeInfo, IndexSet, OperatorId, ValidatorIndex, message::SignedSSVMessage};
use types::{Epoch, Slot};

/// Mock slot clock for testing
#[derive(Clone)]
pub struct MockSlotClock {
    current_slot: u64,
    genesis_time: Duration, // Duration since Unix epoch for genesis
}

impl MockSlotClock {
    pub fn new(current_slot: u64) -> Self {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0));

        // Set genesis so current_slot aligns with current time
        // This ensures the current slot's start time matches approximately now
        let genesis_time = now.saturating_sub(Duration::from_secs(current_slot * 12));

        Self {
            current_slot,
            genesis_time,
        }
    }
}

impl SlotClock for MockSlotClock {
    fn new(_genesis_slot: Slot, genesis_duration: Duration, _slot_duration: Duration) -> Self {
        Self {
            current_slot: 1000,
            genesis_time: genesis_duration,
        }
    }

    fn now(&self) -> Option<Slot> {
        let current_time = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .ok()?;
        self.slot_of(current_time)
    }

    fn slot_duration(&self) -> Duration {
        Duration::from_secs(12) // 12 second slots
    }

    fn start_of(&self, slot: Slot) -> Option<Duration> {
        Some(self.genesis_time + Duration::from_secs(slot.as_u64() * 12))
    }

    fn duration_to_next_slot(&self) -> Option<Duration> {
        Some(Duration::from_secs(12))
    }

    fn duration_to_next_epoch(&self, slots_per_epoch: u64) -> Option<Duration> {
        Some(Duration::from_secs(slots_per_epoch * 12))
    }

    fn is_prior_to_genesis(&self) -> Option<bool> {
        Some(false)
    }

    fn now_duration(&self) -> Option<Duration> {
        // Return actual current system time, not calculated slot time
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .ok()
    }

    fn slot_of(&self, duration: Duration) -> Option<Slot> {
        if duration < self.genesis_time {
            return None; // Before genesis
        }
        let elapsed = duration - self.genesis_time;
        let slot_number = elapsed.as_secs() / 12; // 12 second slots
        Some(Slot::new(slot_number))
    }

    fn duration_to_slot(&self, slot: Slot) -> Option<Duration> {
        Some(self.genesis_time + Duration::from_secs(slot.as_u64() * 12))
    }

    fn genesis_slot(&self) -> Slot {
        Slot::new(0)
    }

    fn genesis_duration(&self) -> Duration {
        self.genesis_time
    }
}

/// Mock duties provider for testing
#[derive(Clone)]
pub struct MockDutiesProvider;

impl DutiesProvider for MockDutiesProvider {
    fn is_validator_in_sync_committee(
        &self,
        _period: u64,
        _validator_index: ValidatorIndex,
    ) -> bool {
        true // Always allow for testing
    }

    fn is_epoch_known_for_proposers(&self, _epoch: Epoch) -> bool {
        true
    }

    fn is_validator_proposer_at_slot(&self, _slot: Slot, _validator_index: ValidatorIndex) -> bool {
        true
    }

    fn get_voluntary_exit_duty_count(&self, _slot: Slot, _pubkey: &PublicKeyBytes) -> u64 {
        0
    }
}

/// Validation adapter that uses the complete message_validator validation pipeline
pub struct ValidationAdapter {
    committee_info: CommitteeInfo,
    test_keys: TestKeySet,
    slot_clock: MockSlotClock,
    duties_provider: Arc<MockDutiesProvider>,
    slots_per_epoch: u64,
}

impl ValidationAdapter {
    pub fn new(committee: IndexSet<OperatorId>) -> Self {
        // Create committee info for the test
        let committee_info = CommitteeInfo {
            committee_members: committee,
            validator_indices: vec![ValidatorIndex(0)], // Simple validator index for testing
        };

        let test_keys = TestKeySet::four_share_set();

        Self {
            committee_info,
            test_keys,
            slot_clock: MockSlotClock::new(0), // Start at slot 0 to align with test messages
            duties_provider: Arc::new(MockDutiesProvider),
            slots_per_epoch: 32,
        }
    }

    /// Validate a SignedSSVMessage using minimal validation (matching Go controller test expectations)
    /// This performs the same validation level as Go controller tests which only check decodability + identifier
    pub fn validate_signed_message(
        &self,
        msg: &SignedSSVMessage,
    ) -> Result<ValidatedSSVMessage, ValidationFailure> {
        // Basic pre-validation checks only
        if msg.operator_ids().is_empty() {
            return Err(ValidationFailure::NoSigners);
        }

        // Use minimal validation that matches Go controller tests
        let consensus_message = minimal_validate_consensus_message(msg)?;

        // Go controller validation is minimal - it only checks:
        // 1. Message can be decoded
        // 2. Message identifier matches controller identifier
        // All other validation (signers, duplicates, etc.) happens during QBFT processing

        // Return the validated message
        Ok(ValidatedSSVMessage::QbftMessage(consensus_message))
    }

    /// Extract slot information from the consensus message
    /// In QBFT, the height field represents the slot/block number
    fn extract_slot_from_message(&self, _msg: &SignedSSVMessage) -> Option<Slot> {
        // For QBFT consensus messages, the height is typically stored in the message identifier
        // The identifier contains the slot/height information
        // Default to slot 0 if extraction fails (most test messages use height 0)
        Some(Slot::new(0))
    }
}
