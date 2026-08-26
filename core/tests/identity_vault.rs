use aether_core::identity::storage::IdentityVault;

#[test]
fn vault_round_trip_preserves_plaintext() {
    let plaintext = b"aether private identity material";
    let secret = b"test-only-secret";

    let encrypted =
        IdentityVault::encrypt(plaintext, secret)
            .expect("vault encryption should succeed");

    assert_ne!(encrypted, plaintext);

    let decrypted =
        IdentityVault::decrypt(&encrypted, secret)
            .expect("vault decryption should succeed");

    assert_eq!(decrypted, plaintext);
}

#[test]
fn vault_rejects_wrong_secret() {
    let plaintext = b"private material";
    let secret = b"correct-secret";

    let encrypted =
        IdentityVault::encrypt(plaintext, secret)
            .expect("vault encryption should succeed");

    let result =
        IdentityVault::decrypt(
            &encrypted,
            b"incorrect-secret",
        );

    assert!(result.is_err());
}

#[test]
fn vault_detects_tampering() {
    let plaintext = b"sensitive identity data";
    let secret = b"vault-secret";

    let mut encrypted =
        IdentityVault::encrypt(plaintext, secret)
            .expect("vault encryption should succeed");

    let index = encrypted.len() - 1;
    encrypted[index] ^= 0x01;

    let result =
        IdentityVault::decrypt(&encrypted, secret);

    assert!(result.is_err());
}

#[test]
fn vault_produces_different_ciphertexts_for_same_plaintext() {
    let plaintext = b"same plaintext";
    let secret = b"same secret";

    let first =
        IdentityVault::encrypt(plaintext, secret)
            .expect("first encryption should succeed");

    let second =
        IdentityVault::encrypt(plaintext, secret)
            .expect("second encryption should succeed");

    assert_ne!(first, second);
}

#[test]
fn vault_rejects_truncated_data() {
    let result =
        IdentityVault::decrypt(
            b"invalid-vault",
            b"secret",
        );

    assert!(result.is_err());
}
