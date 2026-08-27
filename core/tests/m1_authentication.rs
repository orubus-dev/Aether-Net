use aether_core::identity::device::generate_device;

use aether_core::network::NodeId;

use aether_core::runtime::discovery::{PeerChallengeMessage, PeerResponseMessage};

use aether_core::runtime::peers::{AuthenticationState, PeerAuthentication};

use aether_core::link::crypto::EphemeralKey;

use std::time::Instant;

#[test]
fn challenge_round_trip() {
    let (_, device_a) = generate_device(1).expect("device A failed");

    let (_, device_b) = generate_device(1).expect("device B failed");

    let initiator = NodeId::from_device(&device_a);

    let responder = NodeId::from_device(&device_b);

    let ephemeral = EphemeralKey::generate().expect("ephemeral generation failed");

    let message =
        PeerChallengeMessage::new(initiator, responder, ephemeral.public_key().to_bytes(), 100)
            .expect("challenge creation failed");

    let encoded = message.encode();

    let decoded = PeerChallengeMessage::decode(&encoded).expect("challenge decode failed");

    assert_eq!(decoded, message);
}

#[test]
fn signed_response_verifies() {
    let (device_key, device) = generate_device(1).expect("device generation failed");

    let node_id = NodeId::from_device(&device);

    let initiator_ephemeral =
        EphemeralKey::generate().expect("initiator ephemeral generation failed");

    let responder_ephemeral =
        EphemeralKey::generate().expect("responder ephemeral generation failed");

    let challenge = [9u8; 32];

    let response = PeerResponseMessage::sign(
        device_key.signing_key(),
        node_id,
        device.device_id,
        challenge,
        initiator_ephemeral.public_key().to_bytes(),
        responder_ephemeral.public_key().to_bytes(),
        100,
    );

    let public_key = device.public_key().expect("device public key invalid");

    assert!(response.verify(&public_key));
}

#[test]
fn signed_response_rejects_tampering() {
    let (device_key, device) = generate_device(1).expect("device generation failed");

    let node_id = NodeId::from_device(&device);

    let initiator_ephemeral =
        EphemeralKey::generate().expect("initiator ephemeral generation failed");

    let responder_ephemeral =
        EphemeralKey::generate().expect("responder ephemeral generation failed");

    let mut response = PeerResponseMessage::sign(
        device_key.signing_key(),
        node_id,
        device.device_id,
        [3u8; 32],
        initiator_ephemeral.public_key().to_bytes(),
        responder_ephemeral.public_key().to_bytes(),
        100,
    );

    response.timestamp = response.timestamp.saturating_add(1);

    let public_key = device.public_key().expect("device public key invalid");

    assert!(!response.verify(&public_key));
}

#[test]
fn authentication_state_progresses() {
    let (_, device) = generate_device(1).expect("device generation failed");

    let node_id = NodeId::from_device(&device);

    let now = Instant::now();

    let mut auth = PeerAuthentication::new(node_id, now);

    assert_eq!(auth.state, AuthenticationState::Unauthenticated);

    auth.challenge_sent([5u8; 32], now);

    assert_eq!(auth.state, AuthenticationState::ChallengeSent);

    auth.response_received(now);

    assert_eq!(auth.state, AuthenticationState::ResponseReceived);

    auth.verified(now);

    assert_eq!(auth.state, AuthenticationState::Verified);

    auth.established(now);

    assert_eq!(auth.state, AuthenticationState::Established);
}
