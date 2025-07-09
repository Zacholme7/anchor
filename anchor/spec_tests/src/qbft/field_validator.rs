use ssv_types::message::{SignedSSVMessage, SignatureList};
use ssv_types::consensus::*;
use ssv_types::OperatorId;
use ssz::{Encode, Decode};
use tree_hash::TreeHash;
use hex;
use sha2::{Sha256, Digest};
use types::{Hash256, VariableList, 
    typenum::{U13, U836, U388, U1000, U1000000, U8, Sum, Prod}
};

#[derive(Debug)]
pub struct ValidationReport {
    pub test_name: String,
    pub overall_valid: bool,
    pub field_validations: Vec<FieldValidation>,
    pub hash_comparison: HashComparison,
}

#[derive(Debug)]
pub struct FieldValidation {
    pub field_name: String,
    pub valid: bool,
    pub actual_size: usize,
    pub actual_hex: String,
    pub details: String,
}

#[derive(Debug)]
pub struct HashComparison {
    pub expected_hash: String,
    pub actual_hash: String,
    pub matches: bool,
}

pub fn validate_message_construction(
    test_name: &str, 
    rust_msg: &SignedSSVMessage, 
    expected_hash: Hash256
) -> ValidationReport {
    println!("\n🔍 COMPREHENSIVE MESSAGE VALIDATION for '{}'", test_name);
    println!("{}", "=".repeat(80));
    
    let mut field_validations = Vec::new();
    
    // Validate signatures field
    let sig_validation = validate_signatures_field(rust_msg.signatures());
    field_validations.push(sig_validation);
    
    // Validate operator_ids field
    let op_validation = validate_operator_ids_field_slice(rust_msg.operator_ids());
    field_validations.push(op_validation);
    
    // Validate ssv_message field
    let ssv_validation = validate_ssv_message_field(rust_msg.ssv_message());
    field_validations.push(ssv_validation);
    
    // Validate full_data field
    let full_data_validation = validate_full_data_field_slice(rust_msg.full_data());
    field_validations.push(full_data_validation);
    
    // Overall message validation
    let overall_validation = validate_overall_message(rust_msg);
    field_validations.push(overall_validation);
    
    // Hash comparison
    let actual_hash = rust_msg.tree_hash_root();
    let hash_comparison = HashComparison {
        expected_hash: hex::encode(expected_hash),
        actual_hash: hex::encode(actual_hash),
        matches: actual_hash == expected_hash,
    };
    
    println!("\n📊 HASH COMPARISON:");
    println!("Expected: {}", hash_comparison.expected_hash);
    println!("Actual:   {}", hash_comparison.actual_hash);
    println!("Match:    {}", hash_comparison.matches);
    
    let overall_valid = field_validations.iter().all(|v| v.valid) && hash_comparison.matches;
    
    ValidationReport {
        test_name: test_name.to_string(),
        overall_valid,
        field_validations,
        hash_comparison,
    }
}

pub fn validate_signatures_field(signatures: &SignatureList) -> FieldValidation {
    println!("\n🔐 SIGNATURES FIELD VALIDATION:");
    println!("Count: {}", signatures.len());
    
    let encoded = signatures.as_ssz_bytes();
    println!("SSZ Size: {} bytes", encoded.len());
    println!("SSZ Hex (first 100 bytes): {}", hex::encode(&encoded[..encoded.len().min(100)]));
    
    // Detailed signature analysis
    for (i, sig) in signatures.iter().enumerate() {
        let sig_bytes = sig.as_ssz_bytes();
        println!("  Signature {}: {} bytes, hex: {}", i, sig_bytes.len(), hex::encode(&sig_bytes[..sig_bytes.len().min(20)]));
    }
    
    FieldValidation {
        field_name: "signatures".to_string(),
        valid: true, // We'll determine this based on expected behavior
        actual_size: encoded.len(),
        actual_hex: hex::encode(&encoded[..encoded.len().min(50)]),
        details: format!("Contains {} signatures", signatures.len()),
    }
}

pub fn validate_operator_ids_field_slice(operator_ids: &[OperatorId]) -> FieldValidation {
    println!("\n👥 OPERATOR_IDS FIELD VALIDATION:");
    println!("Count: {}", operator_ids.len());
    
    let encoded = VariableList::<OperatorId, U13>::from(operator_ids.to_vec()).as_ssz_bytes();
    println!("SSZ Size: {} bytes", encoded.len());
    println!("SSZ Hex: {}", hex::encode(&encoded));
    
    // Detailed operator ID analysis
    for (i, op_id) in operator_ids.iter().enumerate() {
        println!("  OperatorId {}: value = {}", i, op_id.0);
    }
    
    FieldValidation {
        field_name: "operator_ids".to_string(),
        valid: true,
        actual_size: encoded.len(),
        actual_hex: hex::encode(&encoded),
        details: format!("Contains {} operator IDs: {:?}", operator_ids.len(), operator_ids.iter().map(|op| op.0).collect::<Vec<_>>()),
    }
}

pub fn validate_ssv_message_field(ssv_message: &ssv_types::message::SSVMessage) -> FieldValidation {
    println!("\n📨 SSV_MESSAGE FIELD VALIDATION:");
    
    let encoded = ssv_message.as_ssz_bytes();
    println!("SSZ Size: {} bytes", encoded.len());
    println!("SSZ Hex (first 100 bytes): {}", hex::encode(&encoded[..encoded.len().min(100)]));
    
    // Detailed SSVMessage analysis
    println!("  MsgType: {:?}", ssv_message.msg_type());
    println!("  MessageId: {:?}", ssv_message.msg_id());
    println!("  Data size: {} bytes", ssv_message.data().len());
    println!("  Data hex (first 50 bytes): {}", hex::encode(&ssv_message.data()[..ssv_message.data().len().min(50)]));
    
    // Decode the QbftMessage from data
    if let Ok(qbft_msg) = QbftMessage::from_ssz_bytes(ssv_message.data()) {
        println!("  QBFT Message Type: {:?}", qbft_msg.qbft_message_type);
        println!("  QBFT Height: {}", qbft_msg.height);
        println!("  QBFT Round: {}", qbft_msg.round);
        println!("  QBFT Root: {}", hex::encode(qbft_msg.root));
        println!("  QBFT Data Round: {}", qbft_msg.data_round);
        println!("  QBFT RC Justifications: {}", qbft_msg.round_change_justification.len());
        println!("  QBFT Prepare Justifications: {}", qbft_msg.prepare_justification.len());
        
        // Detailed justification analysis
        for (i, rc_just) in qbft_msg.round_change_justification.iter().enumerate() {
            println!("    RC Just {}: {} bytes", i, rc_just.len());
        }
        for (i, prep_just) in qbft_msg.prepare_justification.iter().enumerate() {
            println!("    Prep Just {}: {} bytes", i, prep_just.len());
        }
    } else {
        println!("  ❌ Failed to decode QbftMessage from SSVMessage data");
    }
    
    FieldValidation {
        field_name: "ssv_message".to_string(),
        valid: true,
        actual_size: encoded.len(),
        actual_hex: hex::encode(&encoded[..encoded.len().min(50)]),
        details: format!("SSVMessage with {} bytes of data", ssv_message.data().len()),
    }
}

pub fn validate_full_data_field_slice(full_data: &[u8]) -> FieldValidation {
    println!("\n💾 FULL_DATA FIELD VALIDATION:");
    println!("Size: {} bytes", full_data.len());
    
    let encoded = VariableList::<u8, Sum<Prod<U8, U1000000>, Sum<Prod<U388, U1000>, U836>>>::from(full_data.to_vec()).as_ssz_bytes();
    println!("SSZ Size: {} bytes", encoded.len());
    
    if !full_data.is_empty() {
        println!("Content hex: {}", hex::encode(&full_data[..full_data.len().min(100)]));
        println!("SSZ Hex: {}", hex::encode(&encoded));
    } else {
        println!("Content: (empty)");
        println!("SSZ Hex: {}", hex::encode(&encoded));
    }
    
    FieldValidation {
        field_name: "full_data".to_string(),
        valid: true,
        actual_size: encoded.len(),
        actual_hex: hex::encode(&encoded),
        details: format!("Contains {} bytes of data", full_data.len()),
    }
}

pub fn validate_overall_message(rust_msg: &SignedSSVMessage) -> FieldValidation {
    println!("\n🏗️ OVERALL MESSAGE VALIDATION:");
    
    let encoded = rust_msg.as_ssz_bytes();
    println!("Total SSZ Size: {} bytes", encoded.len());
    println!("Total SSZ Hex (first 100 bytes): {}", hex::encode(&encoded[..encoded.len().min(100)]));
    println!("Total SSZ Hex (last 50 bytes): {}", hex::encode(&encoded[encoded.len().saturating_sub(50)..]));
    
    // Tree hash validation
    let tree_hash = rust_msg.tree_hash_root();
    println!("Tree Hash: {}", hex::encode(tree_hash));
    
    FieldValidation {
        field_name: "overall_message".to_string(),
        valid: true,
        actual_size: encoded.len(),
        actual_hex: hex::encode(&encoded[..encoded.len().min(50)]),
        details: format!("Complete message with {} bytes SSZ encoding", encoded.len()),
    }
}

pub fn compare_field_encoding<T: Encode>(field: &T, field_name: &str) -> FieldValidation {
    println!("\n🔍 FIELD ENCODING COMPARISON for '{}':", field_name);
    
    let encoded = field.as_ssz_bytes();
    println!("Size: {} bytes", encoded.len());
    println!("Hex: {}", hex::encode(&encoded[..encoded.len().min(100)]));
    
    FieldValidation {
        field_name: field_name.to_string(),
        valid: true,
        actual_size: encoded.len(),
        actual_hex: hex::encode(&encoded[..encoded.len().min(50)]),
        details: format!("Field '{}' encoded to {} bytes", field_name, encoded.len()),
    }
}

pub fn debug_message_step_by_step(rust_msg: &SignedSSVMessage) {
    println!("\n🔬 STEP-BY-STEP MESSAGE ANALYSIS:");
    println!("{}", "=".repeat(80));
    
    // Step 1: Raw field analysis
    println!("\n1️⃣ RAW FIELD ANALYSIS:");
    
    let signatures_bytes = rust_msg.signatures().as_ssz_bytes();
    let operator_ids_bytes = VariableList::<OperatorId, U13>::from(rust_msg.operator_ids().to_vec()).as_ssz_bytes();
    let ssv_message_bytes = rust_msg.ssv_message().as_ssz_bytes();
    let full_data_bytes = VariableList::<u8, Sum<Prod<U8, U1000000>, Sum<Prod<U388, U1000>, U836>>>::from(rust_msg.full_data().to_vec()).as_ssz_bytes();
    
    println!("Signatures:   {} bytes", signatures_bytes.len());
    println!("OperatorIds:  {} bytes", operator_ids_bytes.len());
    println!("SSVMessage:   {} bytes", ssv_message_bytes.len());
    println!("FullData:     {} bytes", full_data_bytes.len());
    
    // Step 2: Field content analysis
    println!("\n2️⃣ FIELD CONTENT ANALYSIS:");
    
    println!("Signatures count: {}", rust_msg.signatures().len());
    println!("OperatorIds count: {}", rust_msg.operator_ids().len());
    println!("FullData size: {} bytes", rust_msg.full_data().len());
    
    // Step 3: SSZ structure analysis
    println!("\n3️⃣ SSZ STRUCTURE ANALYSIS:");
    
    let total_encoded = rust_msg.as_ssz_bytes();
    println!("Total message size: {} bytes", total_encoded.len());
    
    // Parse SSZ offset table
    if total_encoded.len() >= 16 {
        let offset1 = u32::from_le_bytes([total_encoded[0], total_encoded[1], total_encoded[2], total_encoded[3]]) as usize;
        let offset2 = u32::from_le_bytes([total_encoded[4], total_encoded[5], total_encoded[6], total_encoded[7]]) as usize;
        let offset3 = u32::from_le_bytes([total_encoded[8], total_encoded[9], total_encoded[10], total_encoded[11]]) as usize;
        let offset4 = u32::from_le_bytes([total_encoded[12], total_encoded[13], total_encoded[14], total_encoded[15]]) as usize;
        
        println!("SSZ Offset table:");
        println!("  Offset 1 (signatures):  {}", offset1);
        println!("  Offset 2 (operator_ids): {}", offset2);
        println!("  Offset 3 (ssv_message):  {}", offset3);
        println!("  Offset 4 (full_data):    {}", offset4);
        
        // Calculate field sizes from offsets
        let sig_size = offset2 - offset1;
        let op_size = offset3 - offset2;
        let ssv_size = offset4 - offset3;
        let full_size = total_encoded.len() - offset4;
        
        println!("Calculated field sizes:");
        println!("  Signatures field:  {} bytes", sig_size);
        println!("  OperatorIds field: {} bytes", op_size);
        println!("  SSVMessage field:  {} bytes", ssv_size);
        println!("  FullData field:    {} bytes", full_size);
    }
    
    // Step 4: Detailed SSZ byte-by-byte analysis
    println!("\n4️⃣ DETAILED SSZ ENCODING ANALYSIS:");
    debug_ssz_encoding_details(rust_msg);
    
    // Step 5: Tree hash calculation steps
    println!("\n5️⃣ TREE HASH CALCULATION:");
    debug_tree_hash_calculation(rust_msg);
}

pub fn debug_ssz_encoding_details(rust_msg: &SignedSSVMessage) {
    println!("🔍 DETAILED SSZ FIELD ENCODING:");
    
    // Individual field SSZ bytes
    let signatures_ssz = rust_msg.signatures().as_ssz_bytes();
    let operator_ids_ssz = VariableList::<OperatorId, U13>::from(rust_msg.operator_ids().to_vec()).as_ssz_bytes();
    let ssv_message_ssz = rust_msg.ssv_message().as_ssz_bytes();
    let full_data_ssz = VariableList::<u8, Sum<Prod<U8, U1000000>, Sum<Prod<U388, U1000>, U836>>>::from(rust_msg.full_data().to_vec()).as_ssz_bytes();
    
    println!("\n📝 SIGNATURES SSZ ({} bytes):", signatures_ssz.len());
    print_hex_chunks(&signatures_ssz, "SIG");
    
    println!("\n👥 OPERATOR_IDS SSZ ({} bytes):", operator_ids_ssz.len());
    print_hex_chunks(&operator_ids_ssz, "OPS");
    
    println!("\n📨 SSV_MESSAGE SSZ ({} bytes):", ssv_message_ssz.len());
    print_hex_chunks(&ssv_message_ssz, "SSV");
    
    println!("\n💾 FULL_DATA SSZ ({} bytes):", full_data_ssz.len());
    print_hex_chunks(&full_data_ssz, "DAT");
    
    // Complete message SSZ
    let complete_ssz = rust_msg.as_ssz_bytes();
    println!("\n🏗️ COMPLETE MESSAGE SSZ ({} bytes):", complete_ssz.len());
    print_hex_chunks(&complete_ssz, "MSG");
}

pub fn debug_tree_hash_calculation(rust_msg: &SignedSSVMessage) {
    use tree_hash::TreeHash;
    
    println!("🌳 TREE HASH STEP-BY-STEP:");
    
    // Individual field tree hashes
    let sig_hash = rust_msg.signatures().tree_hash_root();
    let ops_hash = VariableList::<OperatorId, U13>::from(rust_msg.operator_ids().to_vec()).tree_hash_root();
    let ssv_hash = rust_msg.ssv_message().tree_hash_root();
    let dat_hash = VariableList::<u8, Sum<Prod<U8, U1000000>, Sum<Prod<U388, U1000>, U836>>>::from(rust_msg.full_data().to_vec()).tree_hash_root();
    
    println!("  Signatures tree hash:  {}", hex::encode(sig_hash));
    println!("  OperatorIds tree hash: {}", hex::encode(ops_hash));
    println!("  SSVMessage tree hash:  {}", hex::encode(ssv_hash));
    println!("  FullData tree hash:    {}", hex::encode(dat_hash));
    
    // Final combined tree hash
    let final_hash = rust_msg.tree_hash_root();
    println!("  📋 FINAL tree hash:     {}", hex::encode(final_hash));
    
    // Manual tree hash construction for debugging
    println!("\n🔧 MANUAL TREE HASH CONSTRUCTION:");
    let mut combined_hashes = Vec::new();
    combined_hashes.extend_from_slice(&sig_hash.0);
    combined_hashes.extend_from_slice(&ops_hash.0);
    combined_hashes.extend_from_slice(&ssv_hash.0);
    combined_hashes.extend_from_slice(&dat_hash.0);
    
    println!("  Combined field hashes ({} bytes): {}", combined_hashes.len(), hex::encode(&combined_hashes));
    
    // Hash the combined hashes
    use sha2::{Sha256, Digest};
    let manual_hash = Sha256::digest(&combined_hashes);
    println!("  Manual hash result: {}", hex::encode(manual_hash));
}

fn print_hex_chunks(data: &[u8], prefix: &str) {
    const CHUNK_SIZE: usize = 32;
    for (i, chunk) in data.chunks(CHUNK_SIZE).enumerate() {
        println!("  {}[{:03}]: {}", prefix, i * CHUNK_SIZE, hex::encode(chunk));
    }
    if data.len() > 200 {
        println!("  {}[...]: ({} more bytes)", prefix, data.len() - 200);
    }
}