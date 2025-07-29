use serde::Deserialize;
use ssv_types::OperatorId;

/// Committee member as defined by the spec. Used for parsing
/// and then covnerted into our internal types
#[derive(Debug, Clone, Deserialize)]
pub struct SpecTestCommitteeMember {
    #[serde(rename = "OperatorID")]
    pub operator_id: OperatorId,
    #[serde(rename = "CommitteeID")]
    pub committee_id: Vec<u8>,
    #[serde(rename = "SSVOperatorPubKey")]
    pub ssv_operator_pub_key: String,
    #[serde(rename = "FaultyNodes")]
    pub faulty_nodes: u64,
    #[serde(rename = "Committee")]
    pub committee: Vec<SpecTestOperator>,
    #[serde(rename = "DomainType")]
    pub domain_type: Vec<u8>,
}

/// Operator from the spec test
#[derive(Debug, Clone, Deserialize)]
pub struct SpecTestOperator {
    #[serde(rename = "OperatorID")]
    pub operator_id: u64,
    #[serde(rename = "SSVOperatorPubKey")]
    pub ssv_operator_pub_key: String,
}
