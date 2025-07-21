use serde::Serialize;
use sha2::{Digest, Sha256};
use ssv_types::{IndexSet, OperatorId};

use super::types::AdapterError;

/// Shared serializable controller structure for consistent JSON formatting
#[derive(Debug, Clone, Serialize)]
pub struct SerializableController {
    #[serde(rename = "Identifier")]
    #[serde(with = "base64_serde")]
    pub identifier: Vec<u8>,
    #[serde(rename = "Height")]
    pub height: u64,
    #[serde(rename = "StoredInstances")]
    pub stored_instances: serde_json::Value,
    #[serde(rename = "CommitteeMember")]
    pub committee_member: SerializableCommitteeMember,
}

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
    use serde::{Deserialize, Deserializer, Serializer};
    use base64::prelude::*;

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
        BASE64_STANDARD.decode(encoded).map_err(serde::de::Error::custom)
    }
}

/// Shared optional base64 serialization utilities
pub mod optional_base64_serde {
    use serde::{Deserialize, Deserializer, Serializer};
    use base64::prelude::*;

    pub fn serialize<S>(opt_bytes: &Option<Vec<u8>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match opt_bytes {
            Some(bytes) => {
                let encoded = BASE64_STANDARD.encode(bytes);
                serializer.serialize_some(&encoded)
            }
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Vec<u8>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt_encoded: Option<String> = Option::deserialize(deserializer)?;
        match opt_encoded {
            Some(encoded) => BASE64_STANDARD
                .decode(encoded)
                .map(Some)
                .map_err(serde::de::Error::custom),
            None => Ok(None),
        }
    }
}

/// Shared hash calculation utilities
pub fn calculate_sha256_hash(data: &[u8]) -> String {
    let hash = Sha256::digest(data);
    hex::encode(hash)
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

/// Shared message structure validation
pub fn validate_message_structure(message: &ssv_types::message::SignedSSVMessage) -> Result<(), String> {
    if message.operator_ids().is_empty() {
        return Err("no signers".to_string());
    }

    if message.signatures().is_empty() {
        return Err("no signatures".to_string());
    }

    if message.signatures().len() != message.operator_ids().len() {
        return Err("number of signatures is different than number of signers".to_string());
    }

    for signature in message.signatures() {
        if signature.is_empty() {
            return Err("empty signature".to_string());
        }
    }

    for operator_id in message.operator_ids() {
        if operator_id.0 == 0 {
            return Err("signer ID 0 not allowed".to_string());
        }
    }

    let mut seen_signers = std::collections::HashSet::new();
    for operator_id in message.operator_ids() {
        if seen_signers.contains(operator_id) {
            return Err("non unique signer".to_string());
        }
        seen_signers.insert(operator_id);
    }

    Ok(())
}