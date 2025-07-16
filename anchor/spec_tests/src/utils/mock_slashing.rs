use crate::types::{SlashableSlots, ValidationError};
use ssv_types::consensus::BeaconVote;
use types::Slot;

pub struct MockSlashingDetector {
    slashable_slots: SlashableSlots,
}

impl MockSlashingDetector {
    pub fn new(slashable_slots: SlashableSlots) -> Self {
        Self { slashable_slots }
    }
    
    pub fn is_attestation_slashable(&self, _beacon_vote: &BeaconVote, duty_slot: Slot) -> bool {
        // Check if any validator in slashable_slots has this slot marked as slashable
        for (_, slots) in &self.slashable_slots {
            if slots.contains(&duty_slot.to_string()) {
                return true;
            }
        }
        false
    }
    
    pub fn is_block_proposal_slashable(&self, duty_slot: Slot) -> bool {
        // Similar logic for block proposals
        for (_, slots) in &self.slashable_slots {
            if slots.contains(&duty_slot.to_string()) {
                return true;
            }
        }
        false
    }
}