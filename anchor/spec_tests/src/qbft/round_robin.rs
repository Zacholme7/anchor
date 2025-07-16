use serde::Deserialize;
use ssv_types::OperatorId;
use super::adapter::types::SpecTestCommitteeMember;
use crate::{QbftSpecTestType, SpecTest, SpecTestType};

/// Round-robin proposer selection algorithm matching Go implementation
fn round_robin_proposer(committee: &[OperatorId], height: u64, round: u64) -> OperatorId {
    let first_round_index = if height == 0 {
        0
    } else {
        (height as usize) % committee.len()
    };
    
    let index = (first_round_index + (round - 1) as usize) % committee.len();
    committee[index]
}

impl SpecTest for RoundRobinTest {
    fn name(&self) -> &str {
        &self.name
    }
    
    fn test_type() -> SpecTestType {
        SpecTestType::Qbft(QbftSpecTestType::RoundRobin)
    }
    
    fn run(&self) -> bool {
        let committee_ids: Vec<OperatorId> = self.share.committee
            .iter()
            .map(|member| OperatorId::from(member.operator_id))
            .collect();
        
        for i in 0..self.heights.len() {
            let height = self.heights[i];
            let round = self.rounds[i];
            let expected_proposer = OperatorId::from(self.proposers[i]);
            
            let actual_proposer = round_robin_proposer(&committee_ids, height, round);
            
            if actual_proposer != expected_proposer {
                eprintln!(
                    "Round robin test '{}' failed at index {}: height={}, round={}, expected={}, got={}",
                    self.name, i, height, round, expected_proposer.0, actual_proposer.0
                );
                return false;
            }
        }
        
        eprintln!("Round robin test '{}' passed all {} test cases", self.name, self.heights.len());
        true
    }
    
    fn setup(&mut self) {
        // No setup needed for round robin tests
    }
}


#[derive(Debug, Clone, Deserialize)]
pub struct RoundRobinTest {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Type")]
    pub test_type: String,
    #[serde(rename = "Documentation")]
    pub documentation: String,
    #[serde(rename = "Share")]
    pub share: SpecTestCommitteeMember,
    #[serde(rename = "Heights")]
    pub heights: Vec<u64>,
    #[serde(rename = "Rounds")]
    pub rounds: Vec<u64>,
    #[serde(rename = "Proposers")]
    pub proposers: Vec<u64>,
}
