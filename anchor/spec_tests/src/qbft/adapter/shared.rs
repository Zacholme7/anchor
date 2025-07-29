use super::types::AdapterError;
use serde::Serialize;
use ssv_types::msgid::MessageId;
use ssv_types::{IndexSet, OperatorId};

/// Shared serializable committee member structure
#[derive(Debug, Clone, Serialize)]
pub struct SerializableCommitteeMember {
    #[serde(rename = "OperatorID")]
    pub operator_id: u64,
    #[serde(rename = "CommitteeID")]
    pub committee_id: Vec<u8>,
    #[serde(rename = "SSVOperatorPubKey")]
    pub ssv_operator_pub_key: String,
    #[serde(rename = "FaultyNodes")]
    pub faulty_nodes: u64,
    #[serde(rename = "Committee")]
    pub committee: Vec<SerializableOperator>,
    #[serde(rename = "DomainType")]
    pub domain_type: Vec<u8>,
}

/// Shared serializable operator structure
#[derive(Debug, Clone, Serialize)]
pub struct SerializableOperator {
    #[serde(rename = "OperatorID")]
    pub operator_id: u64,
    #[serde(rename = "SSVOperatorPubKey")]
    pub ssv_operator_pub_key: String,
}

/// Shared base64 serialization utilities
pub mod base64_serde {
    use base64::prelude::*;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let encoded = BASE64_STANDARD.encode(bytes);
        serializer.serialize_str(&encoded)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        BASE64_STANDARD
            .decode(encoded)
            .map_err(serde::de::Error::custom)
    }
}

/// Shared committee validation logic
pub fn validate_committee_configuration(
    committee: &IndexSet<OperatorId>,
    quorum_threshold: usize,
) -> Result<(), AdapterError> {
    if committee.is_empty() {
        return Err(AdapterError::Config(
            "Committee cannot be empty".to_string(),
        ));
    }

    if quorum_threshold > committee.len() {
        return Err(AdapterError::Config(
            "Quorum threshold exceeds committee size".to_string(),
        ));
    }

    Ok(())
}
