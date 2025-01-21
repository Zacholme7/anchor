use crate::message::*;
use crate::msgid::MsgId;
use crate::{OperatorId, ValidatorIndex};
use ssz_derive::{Decode, Encode};
use ssz::{Decode, DecodeError, Encode};
use std::fmt::Debug;
use std::hash::Hash;
use tree_hash::{PackedEncoding, TreeHash, TreeHashType};
use tree_hash_derive::TreeHash;
use types::typenum::U13;
use types::{
    AggregateAndProof, BeaconBlock, BlindedBeaconBlock, Checkpoint, CommitteeIndex, EthSpec,
    Hash256, PublicKeyBytes, Signature, Slot, SyncCommitteeContribution, VariableList,
};

//                          UnsignedSSVMessage
//            ----------------------------------------------
//            |                                            |
//            |                                            |
//          SSVMessage                                 FullData
//     ---------------------                          ----------
//     |                   |              ValidatorConsensusData/BeaconVote SSZ
//     |                   |
//   MsgType            FullData
//  ---------          -----------
//  ConsensusMsg       QBFTMessage SSZ
//  PartialSigMsg      PartialSignatureMessage SSZ

pub trait Data: Debug + Clone {
    type Hash: Debug + Clone + Eq + Hash;
    fn hash(&self) -> Self::Hash;
}

/// A SSV Message that has not been signed yet.
#[derive(Clone, Debug)]
pub struct UnsignedSSVMessage {
    /// The SSV Message to be send. This is either a consensus message which contains a serialized
    /// QbftMessage, or a partial signature message which contains a PartialSignatureMessage
    pub ssv_message: SSVMessage,
    /// If this is a consensus message, fulldata contains the beacon data that is being agreed upon.
    /// Otherwise, it is empty.
    pub full_data: Vec<u8>,
}

/// A QBFT specific message
#[derive(Clone, Debug)]
pub struct QbftMessage {
    pub qbft_message_type: QbftMessageType,
    pub height: u64,
    pub round: u64,
    pub identifier: MsgId,
    pub root: Hash256,
    // The last round that obtained a prepare quorum
    pub data_round: u64,
    pub round_change_justification: Vec<SignedSSVMessage>, // always without full_data
    pub prepare_justification: Vec<SignedSSVMessage>,      // always without full_data
}

impl QbftMessage {
    /// Do QBFTMessage specific validation
    pub fn validate(&self) -> bool {
        // todo!() what other identification?
        if self.qbft_message_type > QbftMessageType::RoundChange {
            return false;
        }
        true
    }
}

/// Different states the QBFT Message may represent
#[derive(Clone, Debug, PartialEq, PartialOrd)]
pub enum QbftMessageType {
    Proposal = 0,
    Prepare,
    Commit,
    RoundChange,
}

// A partial signature specific message
#[derive(Clone, Debug)]
pub struct PartialSignatureMessage {
    pub partial_signature: Signature,
    pub signing_root: Hash256,
    pub signer: OperatorId,
    pub validator_index: ValidatorIndex,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SszBytes(pub Vec<u8>);

#[derive(Clone, Debug,  PartialEq, Encode)]
pub struct ValidatorConsensusData {
    pub duty: ValidatorDuty,
    pub version: DataVersion,
    pub data_ssz: Vec<u8>,
}

impl Data for ValidatorConsensusData {
    type Hash = Hash256;

    fn hash(&self) -> Self::Hash {
        todo!()
        //self.tree_hash_root()
    }
}

/*

impl TreeHash for SszBytes {
    fn tree_hash_type() -> TreeHashType {
        TreeHashType::List
    }

    fn tree_hash_packed_encoding(&self) -> PackedEncoding {
        todo!()
    }

    fn tree_hash_packing_factor() -> usize {
        todo!()
    }

    fn tree_hash_root(&self) -> tree_hash::Hash256 {
        todo!()
    }
}
*/

#[derive(Clone, Debug, TreeHash, PartialEq, Encode)]
pub struct ValidatorDuty {
    pub r#type: BeaconRole,
    pub pub_key: PublicKeyBytes,
    pub slot: Slot,
    pub validator_index: ValidatorIndex,
    pub committee_index: CommitteeIndex,
    pub committee_length: u64,
    pub committees_at_slot: u64,
    pub validator_committee_index: u64,
    pub validator_sync_committee_indices: VariableList<u64, U13>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BeaconRole(u64);

impl Encode for BeaconRole {
    fn is_ssz_fixed_len() -> bool {
        todo!()
    }

    fn ssz_append(&self, buf: &mut Vec<u8>) {
        todo!()
    }

    fn ssz_fixed_len() -> usize {
        todo!()
    }

    fn ssz_bytes_len(&self) -> usize {
        todo!()
    }
}

pub const BEACON_ROLE_ATTESTER: BeaconRole = BeaconRole(0);
pub const BEACON_ROLE_AGGREGATOR: BeaconRole = BeaconRole(1);
pub const BEACON_ROLE_PROPOSER: BeaconRole = BeaconRole(2);
pub const BEACON_ROLE_SYNC_COMMITTEE: BeaconRole = BeaconRole(3);
pub const BEACON_ROLE_SYNC_COMMITTEE_CONTRIBUTION: BeaconRole = BeaconRole(4);
pub const BEACON_ROLE_VALIDATOR_REGISTRATION: BeaconRole = BeaconRole(5);
pub const BEACON_ROLE_VOLUNTARY_EXIT: BeaconRole = BeaconRole(6);
pub const BEACON_ROLE_UNKNOWN: BeaconRole = BeaconRole(u64::MAX);

impl TreeHash for BeaconRole {
    fn tree_hash_type() -> TreeHashType {
        u64::tree_hash_type()
    }

    fn tree_hash_packed_encoding(&self) -> PackedEncoding {
        self.0.tree_hash_packed_encoding()
    }

    fn tree_hash_packing_factor() -> usize {
        u64::tree_hash_packing_factor()
    }

    fn tree_hash_root(&self) -> tree_hash::Hash256 {
        self.0.tree_hash_root()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DataVersion(u64);

impl Encode for DataVersion {
    fn is_ssz_fixed_len() -> bool {
        todo!()
    }

    fn ssz_append(&self, buf: &mut Vec<u8>) {
        todo!()
    }

    fn ssz_fixed_len() -> usize {
        todo!()
    }

    fn ssz_bytes_len(&self) -> usize {
        todo!()
    }
}

pub const DATA_VERSION_UNKNOWN: DataVersion = DataVersion(0);
pub const DATA_VERSION_PHASE0: DataVersion = DataVersion(1);
pub const DATA_VERSION_ALTAIR: DataVersion = DataVersion(2);
pub const DATA_VERSION_BELLATRIX: DataVersion = DataVersion(3);
pub const DATA_VERSION_CAPELLA: DataVersion = DataVersion(4);
pub const DATA_VERSION_DENEB: DataVersion = DataVersion(5);

impl TreeHash for DataVersion {
    fn tree_hash_type() -> TreeHashType {
        u64::tree_hash_type()
    }

    fn tree_hash_packed_encoding(&self) -> PackedEncoding {
        self.0.tree_hash_packed_encoding()
    }

    fn tree_hash_packing_factor() -> usize {
        u64::tree_hash_packing_factor()
    }

    fn tree_hash_root(&self) -> tree_hash::Hash256 {
        self.0.tree_hash_root()
    }
}

#[derive(Clone, Debug, TreeHash, Encode)]
#[tree_hash(enum_behaviour = "transparent")]
#[ssz(enum_behaviour = "transparent")]
pub enum DataSsz<E: EthSpec> {
    AggregateAndProof(AggregateAndProof<E>),
    BlindedBeaconBlock(BlindedBeaconBlock<E>),
    BeaconBlock(BeaconBlock<E>),
    Contributions(VariableList<Contribution<E>, U13>),
}

#[derive(Clone, Debug, TreeHash, Encode)]
pub struct Contribution<E: EthSpec> {
    pub selection_proof_sig: Signature,
    pub contribution: SyncCommitteeContribution<E>,
}

#[derive(Clone, Debug, TreeHash, PartialEq, Eq, Encode, Decode)]
pub struct BeaconVote {
    pub block_root: Hash256,
    pub source: Checkpoint,
    pub target: Checkpoint,
}

impl Data for BeaconVote {
    type Hash = Hash256;

    fn hash(&self) -> Self::Hash {
        self.tree_hash_root()
    }
}
