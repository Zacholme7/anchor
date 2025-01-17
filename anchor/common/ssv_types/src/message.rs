use crate::msgid::MsgId;
use crate::{OperatorId, ValidatorIndex};
use ssz_derive::{Decode, Encode};
use std::collections::HashSet;
use std::fmt::Debug;
use std::hash::Hash;
use tree_hash::{PackedEncoding, TreeHash, TreeHashType};
use tree_hash_derive::TreeHash;
use types::typenum::U13;
use types::{
    AggregateAndProof, BeaconBlock, BlindedBeaconBlock, Checkpoint, CommitteeIndex, EthSpec,
    Hash256, PublicKeyBytes, Signature, Slot, SyncCommitteeContribution, VariableList,
};
// todo - dear reader, this mainly serves as plain translation of the types found in the go code
// there are a lot of byte[] there, and that got confusing, below should be more readable.
// it needs some work to actually serialize to the same stuff on wire, and I feel like we can name
// the fields better

pub trait Data: Debug + Clone {
    type Hash: Debug + Clone + Eq + Hash;

    fn hash(&self) -> Self::Hash;
}

#[derive(Clone, Debug)]
pub struct SignedSsvMessage {
    pub signatures: Vec<[u8; 256]>,
    pub operator_ids: Vec<OperatorId>,
    pub ssv_message: SsvMessage,
    pub full_data: Option<FullData>, // should this just be serialized???
}


#[derive(Clone, Debug)]
pub struct UnsignedSsvMessage {
    pub ssv_message: SsvMessage,
    pub full_data: Option<FullData>,
}


impl SignedSsvMessage {
    // Validate the signed message
    pub fn validate(&self) -> bool {
        // OperatorID must have at least one element
        if self.operator_ids.is_empty() {
            return false;
        }

        // Note: Len Signers & Operators will only be > 1 after commit aggregation

        // Any OperatorID must not be 0
        if self.operator_ids.iter().any(|&id| *id == 0) {
            return false;
        }

        // The number of signatures and OperatorIDs must be the same
        if self.operator_ids.len() != self.signatures.len() {
            return false;
        }

        // No duplicate signers
        let mut seen_ids = HashSet::with_capacity(self.operator_ids.len());
        for &id in &self.operator_ids {
            if !seen_ids.insert(id) {
                return false;
            }
        }
        true
    }

    pub fn get_consensus_data(&self) -> Option<ValidatorConsensusData> {
        if let Some(FullData::ValidatorConsensusData(data)) = &self.full_data {
            return Some(data.clone());
        }
        None
    }
}

#[derive(Clone, Debug)]
pub struct SsvMessage {
    pub msg_type: MsgType,
    pub msg_id: MsgId,
    pub data: Vec<u8>, // Underlying type is SSVData
}

#[derive(Clone, Debug)]
pub enum MsgType {
    SsvConsensusMsgType,
    SsvPartialSignatureMsgType,
}

#[derive(Clone, Debug)]
pub enum SsvData {
    QbftMessage(QbftMessage),
    PartialSignatureMessage(PartialSignatureMessage),
}

#[derive(Clone, Debug)]
pub struct QbftMessage {
    pub qbft_message_type: QbftMessageType,
    pub height: u64,
    pub round: u64,
    pub identifier: MsgId,

    pub root: Hash256,
    // The last round that obtained a prepare quorum
    pub data_round: u64,
    pub round_change_justification: Vec<SignedSsvMessage>, // always without full_data
    pub prepare_justification: Vec<SignedSsvMessage>,      // always without full_data
}

impl QbftMessage {
    pub fn validate(&self) -> bool {
        // todo!() what other identification?
        if self.qbft_message_type > QbftMessageType::RoundChange {
            return false;
        }
        true
    }
}

#[derive(Clone, Debug, PartialEq, PartialOrd)]
pub enum QbftMessageType {
    Proposal = 0,
    Prepare,
    Commit,
    RoundChange,
}

#[derive(Clone, Debug)]
pub struct PartialSignatureMessage {
    pub partial_signature: Signature,
    pub signing_root: Hash256,
    pub signer: OperatorId,
    pub validator_index: ValidatorIndex,
    // todo!() test this out
    pub full_data: Option<FullData>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FullData {
    ValidatorConsensusData(ValidatorConsensusData),
    BeaconVote(BeaconVote),
}

impl Data for FullData {
    type Hash = Hash256;

    fn hash(&self) -> Self::Hash {
        match self {
            FullData::ValidatorConsensusData(d) => d.hash(),
            FullData::BeaconVote(d) => d.hash()
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SszBytes(pub Vec<u8>);

#[derive(Clone, Debug, TreeHash ,PartialEq)]
pub struct ValidatorConsensusData {
    pub duty: ValidatorDuty,
    pub version: DataVersion,
    pub data_ssz: SszBytes,
}

impl Data for ValidatorConsensusData {
    type Hash = Hash256;

    fn hash(&self) -> Self::Hash {
        self.tree_hash_root()
    }
}

impl TreeHash for SszBytes {
    fn tree_hash_type() -> TreeHashType {
        TreeHashType::List
    }

    fn tree_hash_packed_encoding(&self) -> PackedEncoding {
        todo!()
    }

    fn tree_hash_packing_factor() -> usize {
        1
    }

    fn tree_hash_root(&self) -> tree_hash::Hash256 {
        todo!()
        //tree_hash::Hash256::from_slice(&tree_hash::merkle_root(&self.0.into(), 1))
    }
}

#[derive(Clone, Debug, TreeHash, PartialEq)]
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

#[derive(Clone, Debug, TreeHash, PartialEq, Eq)]
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
