use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};

pub fn sign(signing_key: &SigningKey, message: &[u8]) -> Signature {
    signing_key.sign(message)
}

pub fn verify(public_key: &VerifyingKey, message: &[u8], signature: &Signature) -> bool {
    public_key.verify(message, signature).is_ok()
}

pub fn verify_bytes(public_key: &VerifyingKey, message: &[u8], signature: &[u8; 64]) -> bool {
    let signature = Signature::from_bytes(signature);

    verify(public_key, message, &signature)
}
