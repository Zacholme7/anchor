use serde::Deserialize;
use ssv_types::message::SSVMessage;
use ssz::{Decode, Encode};

use crate::{
    SpecTest, SpecTestType, types::TypesSpecTestType, utils::deserializers::type_parse::*,
};

#[derive(Debug, Deserialize)]
pub struct SSVMessageEncodingTest {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Type")]
    pub test_type: String,
    #[serde(rename = "Documentation")]
    pub documentation: String,
    #[serde(rename = "Data", deserialize_with = "deserialize_base64_to_bytes")]
    pub data: Vec<u8>,
}

impl SpecTest for SSVMessageEncodingTest {
    fn name(&self) -> &str {
        &self.name
    }

    fn setup(&mut self) {
        // No-op
    }

    fn run(&self) -> bool {
        // Decode the SSVMessage from the provided data
        let ssv_message = match SSVMessage::from_ssz_bytes(&self.data) {
            Ok(bv) => bv,
            Err(_) => return false,
        };

        // Test roundtrip encoding
        let re_encoded = ssv_message.as_ssz_bytes();
        match SSVMessage::from_ssz_bytes(&re_encoded) {
            Ok(re_decoded) => re_decoded == ssv_message,
            Err(_) => false,
        }
    }

    fn test_type() -> SpecTestType {
        SpecTestType::Types(TypesSpecTestType::SSVMsgEncoding)
    }
}
