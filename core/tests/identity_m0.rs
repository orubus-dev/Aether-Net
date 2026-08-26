use aether_core::identity::{
    AetherId, AuthorizationGrant, Capability, DeviceCapabilities, DeviceRegistry, DeviceRevocation,
    DeviceSigningKey, DeviceState, GuardianId, IdentityStore, RecoveryApproval,
    RecoveryConfiguration, RecoveryGuardian, RecoveryRequest, RevocationReason,
};

#[test]
fn device_registry_handles_lifecycle() {
    let key = DeviceSigningKey::generate().expect("device key failed");

    let state = DeviceState::from_signing_key(&key, 1, DeviceCapabilities::default());

    let id = state.device_id;

    let mut registry = DeviceRegistry::new();

    assert!(registry.register(state).is_ok());

    assert_eq!(
        registry.status(&id),
        Some(aether_core::identity::device::DeviceStatus::Active)
    );

    registry.suspend(&id).expect("suspend failed");

    assert_eq!(
        registry.status(&id),
        Some(aether_core::identity::device::DeviceStatus::Suspended)
    );

    registry.restore(&id).expect("restore failed");

    registry.revoke(&id).expect("revoke failed");

    assert_eq!(
        registry.status(&id),
        Some(aether_core::identity::device::DeviceStatus::Revoked)
    );
}

#[test]
fn revocation_is_identity_signed() {
    let identity = AetherId::generate().expect("identity failed");

    let device = DeviceSigningKey::generate().expect("device failed");

    let revocation = DeviceRevocation::create(
        identity.signing_key(),
        device.device_id(),
        RevocationReason::Compromised,
        100,
        1,
    );

    assert!(revocation.verify(&identity.public_key()));
}

#[test]
fn recovery_requires_two_unique_guardians() {
    let identity = AetherId::generate().expect("identity failed");

    let guardian_a = DeviceSigningKey::generate().expect("guardian A failed");

    let guardian_b = DeviceSigningKey::generate().expect("guardian B failed");

    let guardian_c = DeviceSigningKey::generate().expect("guardian C failed");

    let guardians = vec![
        RecoveryGuardian {
            id: GuardianId::from_public_key(&guardian_a.verifying_key()),
            public_key: guardian_a.verifying_key().to_bytes(),
            added_at: 1,
        },
        RecoveryGuardian {
            id: GuardianId::from_public_key(&guardian_b.verifying_key()),
            public_key: guardian_b.verifying_key().to_bytes(),
            added_at: 1,
        },
        RecoveryGuardian {
            id: GuardianId::from_public_key(&guardian_c.verifying_key()),
            public_key: guardian_c.verifying_key().to_bytes(),
            added_at: 1,
        },
    ];

    let configuration =
        RecoveryConfiguration::new(guardians).expect("recovery configuration failed");

    let new_device = DeviceSigningKey::generate().expect("new device failed");

    let identity_fingerprint = aether_core::identity::Fingerprint::from_domain(
        b"aether/test-identity/v1",
        identity.public_key().as_bytes(),
    );

    let request = RecoveryRequest {
        identity_fingerprint: *identity_fingerprint.as_bytes(),

        new_device: new_device.device_id(),

        requested_at: 100,
        nonce: [9u8; 32],
    };

    let approval_a = RecoveryApproval::create(
        guardian_a.signing_key(),
        GuardianId::from_public_key(&guardian_a.verifying_key()),
        request.clone(),
    );

    let approval_b = RecoveryApproval::create(
        guardian_b.signing_key(),
        GuardianId::from_public_key(&guardian_b.verifying_key()),
        request,
    );

    assert!(
        aether_core::identity::recovery::recoverable(&configuration, &[approval_a, approval_b],)
            .expect("recovery verification failed")
    );
}

#[test]
fn authorization_expires() {
    let identity = AetherId::generate().expect("identity failed");

    let device = DeviceSigningKey::generate().expect("device failed");

    let grant = AuthorizationGrant::issue(
        identity.signing_key(),
        device.device_id(),
        Capability::Relay,
        100,
        Some(200),
        [1u8; 32],
    );

    assert!(grant.verify(&identity.public_key(), 150));

    assert!(!grant.verify(&identity.public_key(), 200));
}

#[test]
fn identity_store_round_trip_is_cryptographically_stable() {
    let identity = AetherId::generate().expect("identity failed");

    let original = identity.public_key_bytes();

    let stored =
        IdentityStore::serialize_identity(&identity, b"m0-secret").expect("serialization failed");

    let restored =
        IdentityStore::deserialize_identity(&stored, b"m0-secret").expect("restoration failed");

    assert_eq!(original, restored.public_key_bytes());
}
