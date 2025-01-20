use crate::Round;
use ssv_types::message::Data;
use ssv_types::OperatorId;
use std::collections::{HashMap, HashSet};
use types::Hash256;

/// Message container with strong typing and validation
#[derive(Default)]
pub struct MessageContainer<M> {
    /// Messages indexed by round and then by sender
    messages: HashMap<Round, HashMap<OperatorId, M>>,
    /// Track unique values per round
    values_by_round: HashMap<Round, HashSet<Hash256>>,
    /// The quorum size for the qbft instance
    quorum_size: usize,
}

impl<M: Clone + Data<Hash = Hash256>> MessageContainer<M> {
    pub fn new(quorum_size: usize) -> Self {
        Self {
            quorum_size,
            messages: HashMap::new(),
            values_by_round: HashMap::new(),
        }
    }

    pub fn add_message(&mut self, round: Round, sender: OperatorId, msg: &M) -> bool {
        // Check if we already have a message from this sender for this round
        if self
            .messages
            .get(&round)
            .and_then(|msgs| msgs.get(&sender))
            .is_some()
        {
            return false; // Duplicate
        }

        // Add message and track its value
        self.messages
            .entry(round)
            .or_default()
            .insert(sender, msg.clone());

        self.values_by_round
            .entry(round)
            .or_default()
            .insert(msg.hash());

        true
    }

    pub fn has_quorum(&self, round: Round) -> Option<Hash256> {
        let round_messages = self.messages.get(&round)?;

        // Count occurrences of each value
        let mut value_counts: HashMap<Hash256, usize> = HashMap::new();
        for msg in round_messages.values() {
            *value_counts.entry(msg.hash()).or_default() += 1;
        }

        // Find any value that has reached quorum
        value_counts
            .into_iter()
            .find(|(_, count)| *count >= self.quorum_size)
            .map(|(value, _)| value)
    }

    // Count messages for this round
    pub fn num_messages_for_round(&self, round: Round) -> usize {
        self.messages
            .get(&round)
            .map(|msgs| msgs.len())
            .unwrap_or(0)
    }

    // Gets all messages for a specific round
    pub fn get_messages_for_round(&self, round: Round) -> Vec<&M> {
        // If we have messages for this round in our container, return them all
        // If not, return an empty vector
        self.messages
            .get(&round)
            .map(|round_messages| {
                // Convert the values of the HashMap into a Vec
                round_messages.values().collect()
            })
            .unwrap_or_default()
    }

    pub fn get_messages_for_value(&self, round: Round, value: Hash256) -> Vec<&M> {
        self.messages
            .get(&round)
            .map(|msgs| msgs.values().filter(|msg| msg.hash() == value).collect())
            .unwrap_or_default()
    }
}
