use serde::{Deserialize, Serialize};
use ssv_types::{OperatorId, Round, IndexSet};
use qbft::{DefaultLeaderFunction, InstanceHeight, LeaderFunction};

use crate::{QbftSpecTestType, SpecTest, SpecTestType};
use crate::utils::deserializers::type_parse::deserialize_base64_to_bytes;

/// Round Robin test structure matching the JSON format from the spec tests
#[derive(Debug, Serialize, Deserialize)]
pub struct RoundRobinTest {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Share")]
    pub share: CommitteeMember,
    #[serde(rename = "Heights")]
    pub heights: Vec<u64>,
    #[serde(rename = "Rounds")]
    pub rounds: Vec<u64>,
    #[serde(rename = "Proposers")]
    pub proposers: Vec<OperatorId>,
}

/// Committee member structure matching the Go CommitteeMember type
#[derive(Debug, Serialize, Deserialize)]
pub struct CommitteeMember {
    #[serde(rename = "OperatorID")]
    pub operator_id: OperatorId,
    #[serde(rename = "CommitteeID")]
    pub committee_id: Vec<u8>,
    #[serde(rename = "SSVOperatorPubKey")]
    #[serde(deserialize_with = "deserialize_base64_to_bytes")]
    pub ssv_operator_pub_key: Vec<u8>,
    #[serde(rename = "FaultyNodes")]
    pub faulty_nodes: u64,
    #[serde(rename = "Committee")]
    pub committee: Vec<Operator>,
    #[serde(rename = "DomainType")]
    pub domain_type: Vec<u8>,
}

/// Operator structure for committee members
#[derive(Debug, Serialize, Deserialize)]
pub struct Operator {
    #[serde(rename = "OperatorID")]
    pub operator_id: OperatorId,
    #[serde(rename = "SSVOperatorPubKey")]
    #[serde(deserialize_with = "deserialize_base64_to_bytes")]
    pub ssv_operator_pub_key: Vec<u8>,
}

impl SpecTest for RoundRobinTest {
    fn name(&self) -> &str {
        &self.name
    }

    fn run(&self) -> bool {
        // Create committee from share committee members (maintains insertion order)
        let committee: IndexSet<OperatorId> = self.share.committee
            .iter()
            .map(|op| op.operator_id)
            .collect();

        // Use our existing DefaultLeaderFunction
        let leader_fn = DefaultLeaderFunction {};

        // Test each height/round/proposer combination
        for i in 0..self.heights.len() {
            let height = InstanceHeight::from(self.heights[i] as usize);
            let round = Round::from(self.rounds[i]);
            let expected_proposer = self.proposers[i];
            
            // Check if the expected proposer is indeed the leader for this height/round
            let is_leader = leader_fn.leader_function(&expected_proposer, round, height, &committee);
            
            if !is_leader {
                eprintln!(
                    "Round robin test failed for {}: height={}, round={}, expected_proposer={}, committee={:?}",
                    self.name, self.heights[i], self.rounds[i], expected_proposer, committee
                );
                return false;
            }
        }

        true
    }

    fn setup(&mut self) {
        // No setup needed for Round Robin tests
    }

    fn test_type() -> SpecTestType {
        SpecTestType::Qbft(QbftSpecTestType::RoundRobin)
    }
}
