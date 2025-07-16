use std::collections::HashMap;
use ssv_types::consensus::{BeaconVote, ValidatorConsensusData, ValidatorDuty, BeaconRole};
use types::{Epoch, Slot};

#[derive(Debug, Clone)]
pub enum ValidationError {
    SlashableAttestation,
    SourceGreaterThanTarget,
    FarFutureTarget,
    FarFutureDuty,
    WrongValidatorPk,
    WrongValidatorIndex,
    WrongBeaconRoleType,
    InvalidConsensusData,
    DecodingError(String),
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::SlashableAttestation => write!(f, "slashable attestation"),
            Self::SourceGreaterThanTarget => write!(f, "attestation data source >= target"),
            Self::FarFutureTarget => write!(f, "attestation data target epoch is into far future"),
            Self::FarFutureDuty => write!(f, "duty invalid: duty epoch is into far future"),
            Self::WrongValidatorPk => write!(f, "duty invalid: wrong validator pk"),
            Self::WrongValidatorIndex => write!(f, "duty invalid: wrong validator index"),
            Self::WrongBeaconRoleType => write!(f, "duty invalid: wrong beacon role type"),
            Self::InvalidConsensusData => write!(f, "invalid value"),
            Self::DecodingError(msg) => write!(f, "failed decoding consensus data: {}", msg),
        }
    }
}

pub type SlashableSlots = HashMap<String, Vec<String>>;

#[derive(Debug, Clone)]
pub struct ValidationContext {
    pub current_epoch: Epoch,
    pub slots_per_epoch: u64,
    pub slashable_slots: SlashableSlots,
}

impl ValidationContext {
    pub fn new(slashable_slots: SlashableSlots) -> Self {
        Self {
            current_epoch: Epoch::new(100), // Mock current epoch
            slots_per_epoch: 32,
            slashable_slots,
        }
    }
    
    pub fn epoch_at_slot(&self, slot: Slot) -> Epoch {
        Epoch::new(slot.as_u64() / self.slots_per_epoch)
    }
}