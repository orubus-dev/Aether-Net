use aether_core::identity::device::generate_device;
use aether_core::link::{
    ManagedSession, SessionManager, SessionNegotiation, SessionRole, SessionState,
};
use aether_core::network::NodeId;
use std::time::{Duration, Instant};

#[test]
fn session_manager_registers_session() {
    let (_, device_a) = generate_device(1).expect("device generation failed");

    let (_, device_b) = generate_device(1).expect("device generation failed");

    let node_a = NodeId::from_device(&device_a);

    let node_b = NodeId::from_device(&device_b);

    let initiator = SessionNegotiation::create(SessionRole::Initiator).expect("initiator failed");

    let responder = SessionNegotiation::create(SessionRole::Responder).expect("responder failed");

    let initiator_public = initiator.local_ephemeral.public_key();

    let responder_public = responder.local_ephemeral.public_key();

    let transcript: Vec<&[u8]> = vec![
        b"AETHER-LINK-SESSION-MANAGER",
        node_a.as_bytes(),
        node_b.as_bytes(),
        initiator_public.as_bytes(),
        responder_public.as_bytes(),
    ];

    let link = initiator
        .finish(&responder_public, &transcript, node_a, node_b)
        .expect("session creation failed");

    let now = Instant::now();

    let managed = ManagedSession::new(node_b, link, now);

    let session_id = managed.session_id();

    let mut manager = SessionManager::new();

    manager.insert(managed).expect("session insertion failed");

    assert_eq!(manager.len(), 1);
    assert!(manager.get(&session_id).is_some());

    assert_eq!(
        manager.get(&session_id).unwrap().state,
        SessionState::Established
    );
}

#[test]
fn session_expires_after_idle_timeout() {
    let (_, device_a) = generate_device(1).expect("device generation failed");

    let (_, device_b) = generate_device(1).expect("device generation failed");

    let node_a = NodeId::from_device(&device_a);

    let node_b = NodeId::from_device(&device_b);

    let initiator = SessionNegotiation::create(SessionRole::Initiator).expect("initiator failed");

    let responder = SessionNegotiation::create(SessionRole::Responder).expect("responder failed");

    let initiator_public = initiator.local_ephemeral.public_key();

    let responder_public = responder.local_ephemeral.public_key();

    let transcript: Vec<&[u8]> = vec![
        b"AETHER-LINK-EXPIRATION",
        node_a.as_bytes(),
        node_b.as_bytes(),
        initiator_public.as_bytes(),
        responder_public.as_bytes(),
    ];

    let link = initiator
        .finish(&responder_public, &transcript, node_a, node_b)
        .expect("session creation failed");

    let now = Instant::now();

    let mut managed = ManagedSession::new(node_b, link, now);

    managed.idle_timeout = Duration::from_secs(1);

    assert!(!managed.is_expired(now));

    assert!(managed.is_expired(now + Duration::from_secs(2)));
}

#[test]
fn session_manager_expires_old_sessions() {
    let (_, device_a) = generate_device(1).expect("device generation failed");

    let (_, device_b) = generate_device(1).expect("device generation failed");

    let node_a = NodeId::from_device(&device_a);

    let node_b = NodeId::from_device(&device_b);

    let initiator = SessionNegotiation::create(SessionRole::Initiator).expect("initiator failed");

    let responder = SessionNegotiation::create(SessionRole::Responder).expect("responder failed");

    let initiator_public = initiator.local_ephemeral.public_key();

    let responder_public = responder.local_ephemeral.public_key();

    let transcript: Vec<&[u8]> = vec![
        b"AETHER-LINK-MANAGER-EXPIRY",
        node_a.as_bytes(),
        node_b.as_bytes(),
        initiator_public.as_bytes(),
        responder_public.as_bytes(),
    ];

    let link = initiator
        .finish(&responder_public, &transcript, node_a, node_b)
        .expect("session creation failed");

    let now = Instant::now();

    let mut managed = ManagedSession::new(node_b, link, now);

    managed.idle_timeout = Duration::from_secs(1);

    let mut manager = SessionManager::new();

    manager.insert(managed).expect("insertion failed");

    let removed = manager.expire(now + Duration::from_secs(2));

    assert_eq!(removed, 1);
    assert!(manager.is_empty());
}
