use openssl::pkey::{PKey, Private};
use serde::Deserialize;
use ssv_types::{IndexSet, OperatorId, Round, consensus::QbftMessageType, msgid::MessageId};
use ssz::{Decode, Encode};
use tree_hash::TreeHash;
use types::Hash256;
use sha2::{Sha256, Digest};

use super::{SpecQbft, qbft_deserializers::*};
use crate::{
    QbftSpecTestType, SpecTest, SpecTestType, qbft::SignedSSVMessage, utils::test_keys::TestKeySet,
};

impl SpecTest for CreateMessageTest {
    fn name(&self) -> &str {
        &self.name
    }

    // Run the test by constructing the message and verifying its correctness
    fn run(&self) -> bool {
        println!("\n🚀 STARTING TEST: '{}'", self.name);
        println!("{}", "=".repeat(100));
        
        let spec_qbft = self.spec_qbft.as_ref().expect("Setup has been called");
        let key = self.signing_key.as_ref().expect("Setup has been called");
        
        // Print test input details
        println!("\n📋 TEST INPUT ANALYSIS:");
        println!("Message Type: {:?}", self.create_type);
        println!("Root (Value): {}", hex::encode(self.root));
        println!("Round: {:?}", self.round);
        println!("Expected Hash: {}", hex::encode(self.expected_root));
        
        // StateValue analysis
        if let Some(ref state_value) = self.state_value {
            println!("StateValue: {}", state_value);
            if let Ok(decoded) = base64::decode(state_value) {
                println!("StateValue decoded: {} bytes, hex: {}", decoded.len(), hex::encode(&decoded));
            } else {
                println!("StateValue: failed to decode as base64");
            }
        } else {
            println!("StateValue: null");
        }
        
        // Analyze justifications
        if let Some(ref rc_just) = self.round_change_justifications {
            println!("Round Change Justifications: {} items", rc_just.len());
            for (i, just) in rc_just.iter().enumerate() {
                println!("  RC[{}]: signatures={}, operators={}, full_data={} bytes", 
                    i, just.signatures().len(), just.operator_ids().len(), just.full_data().len());
            }
        }
        
        if let Some(ref prep_just) = self.prepare_justifications {
            println!("Prepare Justifications: {} items", prep_just.len());
            for (i, just) in prep_just.iter().enumerate() {
                println!("  Prep[{}]: signatures={}, operators={}, full_data={} bytes", 
                    i, just.signatures().len(), just.operator_ids().len(), just.full_data().len());
            }
        }
        
        // Validate justification data first
        if let Err(validation_error) = self.validate_justification_data() {
            println!("❌ Validation failed for '{}': {}", self.name, validation_error);
            return false;
        }
        let prepare_justifications = if let Some(prepare) = &self.prepare_justifications {
            prepare.clone()
        } else {
            Vec::new()
        };
        let round_change_justifications =
            if let Some(round_change) = &self.round_change_justifications {
                round_change.clone()
            } else {
                Vec::new()
            };

        // Debug: Print justification full_data lengths before creating message
        println!("   🔍 DEBUG: Justifications before create_message:");
        for (i, justification) in round_change_justifications.iter().enumerate() {
            println!("     round_change[{}]: full_data.len() = {}", i, justification.full_data().len());
        }
        for (i, justification) in prepare_justifications.iter().enumerate() {
            println!("     prepare[{}]: full_data.len() = {}", i, justification.full_data().len());
        }

        // Create a new unsigned message. Have to create a new unsigned message to be received on
        // the queue and then perform signing
        println!("\n🏗️ CREATING UNSIGNED MESSAGE:");
        let unsigned_message = spec_qbft.create_message_with_state_value(
            self.create_type,
            self.root,
            self.round,
            round_change_justifications.clone(),
            prepare_justifications.clone(),
            self.state_value.as_deref(),
        );

        println!("✅ Unsigned message created");
        println!("Unsigned SSV Message size: {} bytes", unsigned_message.unsigned_message.ssv_message.as_ssz_bytes().len());
        println!("Unsigned full_data size: {} bytes", unsigned_message.unsigned_message.full_data.len());
        
        println!("\n🔐 SIGNING MESSAGE:");
        let signed_message = spec_qbft.sign(unsigned_message, key);
        
        println!("✅ Message signed");
        
        // Use field validator for comprehensive analysis
        use super::field_validator::*;
        debug_message_step_by_step(&signed_message);
        
        let validation_report = validate_message_construction(&self.name, &signed_message, self.expected_root);
        println!("\n📊 VALIDATION REPORT:");
        println!("Overall Valid: {}", validation_report.overall_valid);
        for field_val in &validation_report.field_validations {
            println!("  {}: {} ({} bytes)", field_val.field_name, 
                if field_val.valid { "✅" } else { "❌" }, field_val.actual_size);
        }

        // Compute the merkle root of the message and compare it to the expected_root
        let result = spec_qbft.verify_root(signed_message.clone(), self.expected_root);

        // If verification failed, load and compare with Go final state
        if !result {
            println!("\n❌ FAILED - Test '{}'", self.name);
            println!(
                "   Rust hash: {}",
                hex::encode(signed_message.tree_hash_root())
            );
            println!("   Expected:  {}", hex::encode(self.expected_root));

            self.compare_with_go_final_state(&signed_message);
        } else {
            println!("✅ PASSED - Test '{}'", self.name);
        }

        result

        // If there are justifications, verify those.. todo!()
    }

    // Setup the qbft instance for constructing a new message
    fn setup(&mut self) {
        let four_share_set = TestKeySet::four_share_set();
        let committee: IndexSet<OperatorId> =
            four_share_set.operator_keys.keys().cloned().collect();

        // All test identifiers are [1,2,3,4]
        let identifier = MessageId::for_spectest();

        // All message creation testing code uses operator one as the message signer
        let operator_one_private = four_share_set
            .operator_keys
            .get(&OperatorId::from(1))
            .expect("Exists");
        let operator_one_private =
            PKey::from_rsa(operator_one_private.to_owned()).expect("Valid key");

        let qbft = SpecQbft::new(committee, identifier);

        // Complete the setup
        self.spec_qbft = Some(qbft);
        self.signing_key = Some(operator_one_private);
    }

    fn test_type() -> SpecTestType {
        SpecTestType::Qbft(QbftSpecTestType::CreateMessage)
    }
}

// Representation of CreateMsgSpecTest files
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateMessageTest {
    // Name of the test that is being run
    #[serde(rename = "Name")]
    pub name: String,

    // Root of the QBFT Message, This is the unhashed ssz bytes of the data
    #[serde(rename = "Value", deserialize_with = "deserialize_value_into_root")]
    pub root: Hash256,

    // The last prepared value of the qbft instance. Todo!() What format is this in??
    #[serde(rename = "StateValue")]
    pub state_value: Option<String>,

    // The round this message is for
    #[serde(rename = "Round", deserialize_with = "deserialize_u64_into_round")]
    pub round: Option<Round>,

    // Any round change justifications for the message
    #[serde(rename = "RoundChangeJustifications")]
    pub round_change_justifications: Option<Vec<SignedSSVMessage>>,

    // Any prepare justifications for the message
    #[serde(rename = "PrepareJustifications")]
    pub prepare_justifications: Option<Vec<SignedSSVMessage>>,

    // The type of the QBFT Message to create
    #[serde(
        rename = "CreateType",
        deserialize_with = "deserialize_qbft_message_type"
    )]
    pub create_type: QbftMessageType,

    // The Expected Root of the QBFT Message
    #[serde(rename = "ExpectedRoot")]
    pub expected_root: Hash256,

    // Any Errors that were expected
    #[serde(rename = "ExpectedError")]
    pub expected_error: String,

    // Qbft Instance that is used for running the test. Skip this during deserialization
    #[serde(skip)]
    pub spec_qbft: Option<SpecQbft>,

    // The operator private key for message signing
    #[serde(skip)]
    pub signing_key: Option<PKey<Private>>,
}

impl CreateMessageTest {
    /// Determine what full_data should be based on message type and justifications
    fn get_expected_full_data(&self) -> Vec<u8> {
        match self.create_type {
            QbftMessageType::Proposal => {
                // For proposals with justifications: extract from justifications
                if let Some(ref round_change_justifications) = self.round_change_justifications {
                    for justification in round_change_justifications {
                        if !justification.full_data().is_empty() {
                            return justification.full_data().to_vec();
                        }
                    }
                }
                
                if let Some(ref prepare_justifications) = self.prepare_justifications {
                    for justification in prepare_justifications {
                        if !justification.full_data().is_empty() {
                            return justification.full_data().to_vec();
                        }
                    }
                }
                
                // For simple proposals: use Value field (32 bytes)
                self.root.as_slice().to_vec()
            }
            _ => {
                // For other message types: empty
                Vec::new()
            }
        }
    }

    /// Validate that justifications have expected format
    fn validate_justification_data(&self) -> Result<(), String> {
        // Validate round change justifications
        if let Some(ref round_change_justifications) = self.round_change_justifications {
            for (i, justification) in round_change_justifications.iter().enumerate() {
                if justification.signatures().is_empty() {
                    return Err(format!("Round change justification {i} has no signatures"));
                }
                if justification.operator_ids().is_empty() {
                    return Err(format!("Round change justification {i} has no operator IDs"));
                }
                
                // Check if full_data is base64 decodable (if not empty)
                if !justification.full_data().is_empty() {
                    let full_data = justification.full_data();
                    if full_data.len() > 100 { // Reasonable max size check
                        return Err(format!("Round change justification {i} has unexpectedly large full_data: {} bytes", full_data.len()));
                    }
                }
            }
        }
        
        // Validate prepare justifications
        if let Some(ref prepare_justifications) = self.prepare_justifications {
            for (i, justification) in prepare_justifications.iter().enumerate() {
                if justification.signatures().is_empty() {
                    return Err(format!("Prepare justification {i} has no signatures"));
                }
                if justification.operator_ids().is_empty() {
                    return Err(format!("Prepare justification {i} has no operator IDs"));
                }
                
                // Check if full_data is reasonable
                if !justification.full_data().is_empty() {
                    let full_data = justification.full_data();
                    if full_data.len() > 100 { // Reasonable max size check
                        return Err(format!("Prepare justification {i} has unexpectedly large full_data: {} bytes", full_data.len()));
                    }
                }
            }
        }
        
        Ok(())
    }

    fn compare_with_go_final_state(&self, rust_msg: &SignedSSVMessage) {
        // Try to load the corresponding Go final state file
        let go_final_state_path = self.get_go_final_state_path();
        println!("   🔍 DEBUG: Looking for Go final state at: {}", go_final_state_path);

        match std::fs::read_to_string(&go_final_state_path) {
            Ok(go_json) => {
                println!("   🔍 DEBUG: Successfully loaded Go final state file");
                match serde_json::from_str::<SignedSSVMessage>(&go_json) {
                    Ok(go_final_state) => {
                        println!("   🔍 DEBUG: Successfully parsed Go final state JSON as SignedSSVMessage");
                        self.detailed_comparison_with_go_message(&go_final_state, rust_msg);
                    }
                    Err(e) => {
                        println!("   ❌ Failed to parse Go final state JSON as SignedSSVMessage: {e}");
                        // Try fallback to old format for backward compatibility
                        match serde_json::from_str::<CreateMessageTest>(&go_json) {
                            Ok(go_final_state) => {
                                println!("   🔍 DEBUG: Successfully parsed Go final state JSON as CreateMessageTest (fallback)");
                                self.detailed_comparison(&go_final_state, rust_msg, &go_final_state_path);
                            }
                            Err(e2) => {
                                println!("   ❌ Failed to parse Go final state JSON in any format: {e2}");
                            }
                        }
                    }
                }
            },
            Err(e) => {
                println!("   ❌ No Go comparison state available: {e}");
            }
        }
    }

    fn get_original_test_file_path(&self) -> String {
        // Convert test name to the original Go test file name format
        let sanitized_name = self.name.replace(" ", "_");
        format!(
            "ssv-spec/qbft/spectest/generate/tests/tests.CreateMsgSpecTest_qbft_create_message_{sanitized_name}.json"
        )
    }

    fn get_go_final_state_path(&self) -> String {
        // Convert test name to the Go file name format
        let sanitized_name = self.name.clone(); // Go uses spaces in filenames
        format!(
            "ssv-spec/qbft/spectest/generate/state_comparison/tests_CreateMsgSpecTest/qbft create message {sanitized_name}.json"
        )
    }

    fn list_available_go_files(&self) {
        let dir_path = "ssv-spec/qbft/spectest/generate/state_comparison/tests_CreateMsgSpecTest/";
        if let Ok(entries) = std::fs::read_dir(dir_path) {
            println!("Available Go state files:");
            for entry in entries.flatten() {
                if let Some(filename) = entry.file_name().to_str() {
                    if filename.ends_with(".json") {
                        println!("  - {filename}");
                    }
                }
            }
        }

        // Also try direct name mapping
        let direct_path = format!(
            "ssv-spec/qbft/spectest/generate/state_comparison/tests_CreateMsgSpecTest/{}.json",
            self.name
        );
        println!("Also tried: {direct_path}");
    }

    fn detailed_comparison(
        &self,
        go_state: &CreateMessageTest,
        rust_msg: &SignedSSVMessage,
        _go_file_path: &str,
    ) {
        // Decode Rust QBFT message
        if let Ok(rust_qbft_msg) =
            ssv_types::consensus::QbftMessage::from_ssz_bytes(rust_msg.ssv_message().data())
        {
            let mut mismatches = Vec::new();

            // Check key fields for mismatches

            // Message Type
            let go_create_type_str = format!("{:?}", go_state.create_type);
            let rust_msg_type_str = format!("{:?}", rust_qbft_msg.qbft_message_type);
            if rust_msg_type_str != go_create_type_str {
                mismatches.push(format!(
                    "QbftMessage.msg_type: Rust={rust_msg_type_str} vs Go={go_create_type_str}"
                ));
            }

            // Round comparison
            let go_round = go_state.round.map(u64::from).unwrap_or(0);
            let rust_round = rust_qbft_msg.round;
            if rust_round != go_round {
                mismatches.push(format!(
                    "QbftMessage.round: Rust={rust_round} vs Go={go_round}"
                ));
            }

            // Height (should be 0 for tests)
            if rust_qbft_msg.height != 0 {
                mismatches.push(format!(
                    "QbftMessage.height: Rust={} vs Go=0",
                    rust_qbft_msg.height
                ));
            }

            // Root comparison
            if rust_qbft_msg.root != go_state.root {
                mismatches.push(format!(
                    "QbftMessage.root: Rust={} vs Go={}",
                    hex::encode(rust_qbft_msg.root),
                    hex::encode(go_state.root)
                ));
            }

            // FullData comparison - enhanced debugging
            let expected_full_data = self.get_expected_full_data();
            let actual_full_data = rust_msg.full_data();
            
            println!("   🔍 FULL DATA ANALYSIS:");
            println!("      Message Type: {:?}", self.create_type);
            println!("      Expected full_data length: {}", expected_full_data.len());
            println!("      Actual full_data length: {}", actual_full_data.len());
            
            // Debug: Print justification full_data lengths
            println!("      🔍 DEBUG: Justifications from test JSON:");
            if let Some(ref justifications) = self.round_change_justifications {
                for (i, justification) in justifications.iter().enumerate() {
                    println!("        JSON round_change[{}]: full_data.len() = {}", i, justification.full_data().len());
                }
            }
            if let Some(ref justifications) = self.prepare_justifications {
                for (i, justification) in justifications.iter().enumerate() {
                    println!("        JSON prepare[{}]: full_data.len() = {}", i, justification.full_data().len());
                }
            }
            
            if expected_full_data.len() != actual_full_data.len() {
                mismatches.push(format!(
                    "SignedSSVMessage.full_data.length: Rust={} vs Expected={}",
                    actual_full_data.len(),
                    expected_full_data.len()
                ));
            }
            
            if expected_full_data != actual_full_data {
                mismatches.push(format!(
                    "SignedSSVMessage.full_data.content: Rust={} vs Expected={}",
                    hex::encode(actual_full_data),
                    hex::encode(&expected_full_data)
                ));
            }
            
            // Show MARSHALED justification analysis (from the actual QBFT message)
            println!("      Marshaled round change justifications: {} items", rust_qbft_msg.round_change_justification.len());
            for (i, justification_bytes) in rust_qbft_msg.round_change_justification.iter().enumerate() {
                if let Ok(justification) = SignedSSVMessage::from_ssz_bytes_without_full_data(justification_bytes) {
                    println!("        [{}]: full_data.len() = {}", i, justification.full_data().len());
                } else {
                    println!("        [{}]: failed to unmarshal", i);
                }
                // Show the first 100 bytes of the marshaled justification for debugging
                let bytes_to_show = std::cmp::min(100, justification_bytes.len());
                println!("        [{}]: first {} bytes: {}", i, bytes_to_show, hex::encode(&justification_bytes[..bytes_to_show]));
            }
            
            println!("      Marshaled prepare justifications: {} items", rust_qbft_msg.prepare_justification.len());
            for (i, justification_bytes) in rust_qbft_msg.prepare_justification.iter().enumerate() {
                if let Ok(justification) = SignedSSVMessage::from_ssz_bytes_without_full_data(justification_bytes) {
                    println!("        [{}]: full_data.len() = {}", i, justification.full_data().len());
                } else {
                    println!("        [{}]: failed to unmarshal", i);
                }
            }

            // Justifications count
            let go_rc_count = go_state
                .round_change_justifications
                .as_ref()
                .map(|v| v.len())
                .unwrap_or(0);
            let go_prep_count = go_state
                .prepare_justifications
                .as_ref()
                .map(|v| v.len())
                .unwrap_or(0);

            if rust_qbft_msg.round_change_justification.len() != go_rc_count {
                mismatches.push(format!(
                    "QbftMessage.round_change_justification.length: Rust={} vs Go={}",
                    rust_qbft_msg.round_change_justification.len(),
                    go_rc_count
                ));
            }

            if rust_qbft_msg.prepare_justification.len() != go_prep_count {
                mismatches.push(format!(
                    "QbftMessage.prepare_justification.length: Rust={} vs Go={}",
                    rust_qbft_msg.prepare_justification.len(),
                    go_prep_count
                ));
            }

            // Show mismatches if any
            if !mismatches.is_empty() {
                println!("\n🔍 KEY MISMATCHES DETECTED:");
                println!("   📋 Structure: SignedSSVMessage → SSVMessage → QbftMessage");
                for mismatch in mismatches {
                    println!("   ❌ {mismatch}");
                }
            } else {
                println!("\n🔍 NO FIELD MISMATCHES DETECTED - but hashes still differ");
                println!("   This suggests a subtle serialization or tree hash issue");
                
                // Show the raw tree hash components for debugging
                println!("   🔍 Raw QBFT Message fields:");
                println!("     msg_type: {:?}", rust_qbft_msg.qbft_message_type);
                println!("     height: {}", rust_qbft_msg.height);
                println!("     round: {}", rust_qbft_msg.round);
                println!("     identifier: {}", hex::encode(&rust_qbft_msg.identifier[..]));
                println!("     root: {}", hex::encode(rust_qbft_msg.root));
                println!("     data_round: {}", rust_qbft_msg.data_round);
                println!("     round_change_justification.len(): {}", rust_qbft_msg.round_change_justification.len());
                println!("     prepare_justification.len(): {}", rust_qbft_msg.prepare_justification.len());
                
                // Show Expected vs Actual tree hash
                println!("   🔍 Tree hash comparison:");
                println!("     Rust tree hash: {}", hex::encode(rust_msg.tree_hash_root()));
                println!("     Expected hash:  {}", hex::encode(go_state.expected_root));
                
                // Show the SSZ bytes of the SignedSSVMessage for comparison
                let rust_ssz_bytes = rust_msg.as_ssz_bytes();
                let bytes_to_show = std::cmp::min(200, rust_ssz_bytes.len());
                println!("   🔍 SignedSSVMessage SSZ bytes (first {} bytes):", bytes_to_show);
                println!("     {}", hex::encode(&rust_ssz_bytes[..bytes_to_show]));
                println!("     Total SSZ length: {}", rust_ssz_bytes.len());
            }

            // Check for specific FullData mismatches in justifications
            self.check_justification_mismatches(&rust_qbft_msg, go_state);
        } else {
            println!("   ❌ Failed to decode Rust QBFT message from SSVMessage data");
        }
    }

    fn detailed_comparison_with_go_message(&self, go_msg: &SignedSSVMessage, rust_msg: &SignedSSVMessage) {
        println!("   🔍 DIRECT GO vs RUST MESSAGE COMPARISON:");
        
        // Compare overall structure
        println!("   📋 Structure Comparison:");
        println!("     Signatures count: Go={}, Rust={}", go_msg.signatures().len(), rust_msg.signatures().len());
        println!("     OperatorIDs count: Go={}, Rust={}", go_msg.operator_ids().len(), rust_msg.operator_ids().len());
        println!("     FullData length: Go={}, Rust={}", go_msg.full_data().len(), rust_msg.full_data().len());
        
        // Decode both QBFT messages
        let go_qbft_result = ssv_types::consensus::QbftMessage::from_ssz_bytes(go_msg.ssv_message().data());
        let rust_qbft_result = ssv_types::consensus::QbftMessage::from_ssz_bytes(rust_msg.ssv_message().data());
        
        match (go_qbft_result, rust_qbft_result) {
            (Ok(go_qbft), Ok(rust_qbft)) => {
                println!("   🔍 QBFT Message Field Comparison:");
                println!("     Message Type: Go={:?}, Rust={:?}", go_qbft.qbft_message_type, rust_qbft.qbft_message_type);
                println!("     Height: Go={}, Rust={}", go_qbft.height, rust_qbft.height);
                println!("     Round: Go={}, Rust={}", go_qbft.round, rust_qbft.round);
                println!("     Root: Go={}, Rust={}", hex::encode(go_qbft.root), hex::encode(rust_qbft.root));
                println!("     Data Round: Go={}, Rust={}", go_qbft.data_round, rust_qbft.data_round);
                println!("     RC Justifications: Go={}, Rust={}", go_qbft.round_change_justification.len(), rust_qbft.round_change_justification.len());
                println!("     Prepare Justifications: Go={}, Rust={}", go_qbft.prepare_justification.len(), rust_qbft.prepare_justification.len());
                
                // Check for key differences
                let mut mismatches = Vec::new();
                
                if go_qbft.qbft_message_type != rust_qbft.qbft_message_type {
                    mismatches.push(format!("Message Type: Go={:?} vs Rust={:?}", go_qbft.qbft_message_type, rust_qbft.qbft_message_type));
                }
                if go_qbft.height != rust_qbft.height {
                    mismatches.push(format!("Height: Go={} vs Rust={}", go_qbft.height, rust_qbft.height));
                }
                if go_qbft.round != rust_qbft.round {
                    mismatches.push(format!("Round: Go={} vs Rust={}", go_qbft.round, rust_qbft.round));
                }
                if go_qbft.root != rust_qbft.root {
                    mismatches.push(format!("Root: Go={} vs Rust={}", hex::encode(go_qbft.root), hex::encode(rust_qbft.root)));
                }
                if go_qbft.data_round != rust_qbft.data_round {
                    mismatches.push(format!("Data Round: Go={} vs Rust={}", go_qbft.data_round, rust_qbft.data_round));
                }
                if go_qbft.round_change_justification.len() != rust_qbft.round_change_justification.len() {
                    mismatches.push(format!("RC Justifications count: Go={} vs Rust={}", go_qbft.round_change_justification.len(), rust_qbft.round_change_justification.len()));
                }
                if go_qbft.prepare_justification.len() != rust_qbft.prepare_justification.len() {
                    mismatches.push(format!("Prepare Justifications count: Go={} vs Rust={}", go_qbft.prepare_justification.len(), rust_qbft.prepare_justification.len()));
                }
                
                if mismatches.is_empty() {
                    println!("   ✅ All QBFT message fields match!");
                } else {
                    println!("   ❌ QBFT Message Field Mismatches:");
                    for mismatch in mismatches {
                        println!("     - {}", mismatch);
                    }
                }
                
                // Compare tree hashes
                let go_tree_hash = go_msg.tree_hash_root();
                let rust_tree_hash = rust_msg.tree_hash_root();
                println!("   🌳 Tree Hash Comparison:");
                println!("     Go:   {}", hex::encode(go_tree_hash));
                println!("     Rust: {}", hex::encode(rust_tree_hash));
                println!("     Expected: {}", hex::encode(self.expected_root));
                println!("     Go matches expected: {}", go_tree_hash == self.expected_root);
                println!("     Rust matches expected: {}", rust_tree_hash == self.expected_root);
                println!("     Go matches Rust: {}", go_tree_hash == rust_tree_hash);
            }
            (Err(go_err), _) => println!("   ❌ Failed to decode Go QBFT message: {:?}", go_err),
            (_, Err(rust_err)) => println!("   ❌ Failed to decode Rust QBFT message: {:?}", rust_err),
        }
    }

    fn check_justification_mismatches(
        &self,
        rust_qbft_msg: &ssv_types::consensus::QbftMessage,
        go_state: &CreateMessageTest,
    ) {
        // Check RoundChange justifications for FullData mismatches
        if let Some(go_rc_justifications) = &go_state.round_change_justifications {
            let mut rc_mismatches = 0;
            for (rust_rc_bytes, _go_rc) in rust_qbft_msg
                .round_change_justification
                .iter()
                .zip(go_rc_justifications.iter())
            {
                if let Ok(rust_rc) = SignedSSVMessage::from_ssz_bytes_without_full_data(rust_rc_bytes) {
                    // Marshaled justifications should ALWAYS have empty full_data
                    // The original test JSON might have non-empty full_data, but after marshaling
                    // using WithoutFullData(), they should be empty
                    if rust_rc.full_data().len() != 0 {
                        rc_mismatches += 1;
                    }
                }
            }
            if rc_mismatches > 0 {
                println!(
                    "   ❌ QbftMessage.round_change_justification[*].full_data.length: {rc_mismatches} items have non-empty full_data (should be empty)"
                );
            }
        }

        // Check Prepare justifications for FullData mismatches
        if let Some(go_prep_justifications) = &go_state.prepare_justifications {
            let mut prep_mismatches = 0;
            for (rust_prep_bytes, _go_prep) in rust_qbft_msg
                .prepare_justification
                .iter()
                .zip(go_prep_justifications.iter())
            {
                if let Ok(rust_prep) = SignedSSVMessage::from_ssz_bytes_without_full_data(rust_prep_bytes) {
                    // Marshaled justifications should ALWAYS have empty full_data
                    if rust_prep.full_data().len() != 0 {
                        prep_mismatches += 1;
                    }
                }
            }
            if prep_mismatches > 0 {
                println!(
                    "   ❌ QbftMessage.prepare_justification[*].full_data.length: {prep_mismatches} items have non-empty full_data (should be empty)"
                );
            }
        }
    }
}
