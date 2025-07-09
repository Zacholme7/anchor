// Test to verify the exact bytes produced by the Rust implementation
use hex;
use ssv_types::{message::{SignedSSVMessage, SSVMessage, MsgType}, OperatorId, msgid::MessageId};

fn main() {
    // Create a test message similar to the Go test
    let full_data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 1, 2, 3, 4, 5, 6, 7, 8, 9, 1, 2, 3, 4, 5, 6, 7, 8, 9];
    
    let ssv_message = SSVMessage {
        msg_type: MsgType::SSVConsensusMsgType,
        msg_id: MessageId::default(),
        data: vec![0u8; 400], // Some test data
    };
    
    let signed_message = SignedSSVMessage::new_from_vecs(
        vec![[0u8; 256]], // Single signature 
        vec![OperatorId::from(1)], // Single operator ID
        ssv_message,
        full_data.clone(),
    ).expect("Valid signed message");
    
    println!("Original message FullData length: {}", signed_message.full_data().len());
    println!("Original message FullData: {}", hex::encode(signed_message.full_data()));
    
    // Test encode_without_full_data
    let without_full_data_marshaled = signed_message.encode_without_full_data();
    println!("Without full data marshaled length: {}", without_full_data_marshaled.len());
    
    // Print first 100 bytes
    println!("First 100 bytes of without full data: {}", hex::encode(&without_full_data_marshaled[..100]));
    
    // Test what the first 16 bytes represent (the 4 offsets)
    if without_full_data_marshaled.len() >= 16 {
        let offset1 = u32::from_le_bytes([without_full_data_marshaled[0], without_full_data_marshaled[1], without_full_data_marshaled[2], without_full_data_marshaled[3]]);
        let offset2 = u32::from_le_bytes([without_full_data_marshaled[4], without_full_data_marshaled[5], without_full_data_marshaled[6], without_full_data_marshaled[7]]);
        let offset3 = u32::from_le_bytes([without_full_data_marshaled[8], without_full_data_marshaled[9], without_full_data_marshaled[10], without_full_data_marshaled[11]]);
        let offset4 = u32::from_le_bytes([without_full_data_marshaled[12], without_full_data_marshaled[13], without_full_data_marshaled[14], without_full_data_marshaled[15]]);
        
        println!("Offset 1: {}", offset1);
        println!("Offset 2: {}", offset2);
        println!("Offset 3: {}", offset3);
        println!("Offset 4: {}", offset4);
        println!("Total length: {}", without_full_data_marshaled.len());
    }
}