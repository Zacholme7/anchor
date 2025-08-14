use openssl::hash::MessageDigest;
use openssl::pkey::{PKey, Private};
use openssl::rsa::Rsa;
use openssl::sign::Signer;
use ssz::Encode;

/// Sign a message with RSA key using SHA256
pub fn sign_message_with_rsa(
    message_bytes: &[u8],
    rsa_key: &Rsa<Private>,
) -> Result<[u8; 256], String> {
    let pkey =
        PKey::from_rsa(rsa_key.clone()).map_err(|e| format!("Failed to create PKey: {:?}", e))?;

    let mut signer = Signer::new(MessageDigest::sha256(), &pkey)
        .map_err(|e| format!("Failed to create signer: {:?}", e))?;

    signer
        .update(message_bytes)
        .map_err(|e| format!("Failed to update signer: {:?}", e))?;

    let mut sig = [0u8; 256];
    let len = signer
        .sign(&mut sig)
        .map_err(|e| format!("Failed to sign message: {:?}", e))?;

    if len != 256 {
        return Err(format!("Signature length {} != 256", len));
    }

    Ok(sig)
}

/// Sign an SSZ-encodable message with RSA key
pub fn sign_ssz_message_with_rsa<T: Encode>(
    message: &T,
    rsa_key: &Rsa<Private>,
) -> Result<[u8; 256], String> {
    sign_message_with_rsa(&message.as_ssz_bytes(), rsa_key)
}
