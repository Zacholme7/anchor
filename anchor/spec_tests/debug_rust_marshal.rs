use hex;
use ssv_types::message::SignedSSVMessage;
use ssv_types::OperatorId;
use ssv_types::message::{MsgType, SSVMessage};
use ssv_types::msgid::MessageId;
use ssz::Encode;

fn main() {
    // Create a simple SignedSSVMessage with 27 bytes of full data
    let full_data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 1, 2, 3, 4, 5, 6, 7, 8, 9, 1, 2, 3, 4, 5, 6, 7, 8, 9];
    
    let ssv_message = SSVMessage {
        msg_type: MsgType::SSVConsensusMsgType,
        msg_id: MessageId::default(),
        data: vec![0u8; 100], // Some test data
    };
    
    let signed_message = SignedSSVMessage::new_from_vecs(
        vec![[0u8; 256]], // Single signature
        vec![OperatorId::from(1)], // Single operator ID
        ssv_message,
        full_data.clone(),
    ).expect("Valid signed message");
    
    println!("Original message FullData length: {}", signed_message.full_data().len());
    println!("Original message FullData: {}", hex::encode(signed_message.full_data()));
    
    // Marshal with full data
    let full_data_marshaled = signed_message.as_ssz_bytes();
    println!("Full data marshaled length: {}", full_data_marshaled.len());
    
    // Marshal without full data using our method
    let without_full_data_marshaled = signed_message.encode_without_full_data();
    println!("Without full data marshaled length: {}", without_full_data_marshaled.len());
    
    // Print first 100 bytes for comparison
    println!("\nFirst 100 bytes of without full data: {}", hex::encode(&without_full_data_marshaled[..100]));
    
    // Test the marshal_justifications function
    let messages = vec![signed_message];
    let justifications = super::marshal_justifications(&messages).expect("Marshal should succeed");
    
    println!("Justification marshaled length: {}", justifications[0].len());
    println!("Justification matches without full data: {}", justifications[0].len() == without_full_data_marshaled.len());
    
    // Check if they are identical
    let identical = justifications[0] == without_full_data_marshaled;
    println!("Bytes are identical: {}", identical);
}