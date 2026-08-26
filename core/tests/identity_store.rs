use aether_core::identity::{AetherId, IdentityStore};

#[test]
fn identity_store_round_trip_preserves_identity() {
    let identity = AetherId::generate().expect("identity generation failed");

    let public_key = identity.public_key_bytes();

    let encrypted = IdentityStore::serialize_identity(&identity, b"test-vault-secret")
        .expect("identity serialization failed");

    let restored = IdentityStore::deserialize_identity(&encrypted, b"test-vault-secret")
        .expect("identity restoration failed");

    assert_eq!(public_key, restored.public_key_bytes());
}

#[test]
fn identity_store_rejects_wrong_secret() {
    let identity = AetherId::generate().expect("identity generation failed");

    let encrypted = IdentityStore::serialize_identity(&identity, b"correct-secret")
        .expect("identity serialization failed");

    assert!(IdentityStore::deserialize_identity(&encrypted, b"wrong-secret",).is_err());
}
