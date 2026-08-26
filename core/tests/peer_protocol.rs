use aether_core::identity::device::generate_device;
use aether_core::network::{NodeId, Peer, PeerChallenge, PeerResponse, PeerState};

#[test]
fn node_hello_can_be_created_from_device() {
    let (_, device) = generate_device(1_000).expect("device creation failed");

    let capabilities = aether_core::network::NodeCapabilities::NONE
        .insert(aether_core::network::NodeCapabilities::SEND)
        .insert(aether_core::network::NodeCapabilities::RECEIVE)
        .insert(aether_core::network::NodeCapabilities::RELAY);

    let hello = aether_core::network::NodeHello::from_device(
        &device,
        capabilities,
        aether_core::network::NodeRole::Device,
        1_000,
    )
    .expect("hello creation failed");

    assert_eq!(
        hello.protocol,
        aether_core::network::ProtocolVersion::CURRENT
    );

    assert_eq!(hello.device_id, device.device_id);
}

#[test]
fn peer_challenge_is_fresh() {
    let first = PeerChallenge::generate(1_000).expect("challenge generation failed");

    let second = PeerChallenge::generate(1_000).expect("challenge generation failed");

    assert_ne!(first.challenge, second.challenge);
}

#[test]
fn peer_can_sign_and_verify_challenge() {
    let (device_key, device) = generate_device(1_000).expect("device generation failed");

    let challenge = PeerChallenge::generate(1_000).expect("challenge generation failed");

    let node_id = NodeId::from_device(&device);

    let response = PeerResponse::sign(
        device_key.signing_key(),
        node_id,
        device.device_id,
        &challenge,
    );

    let public_key = device.public_key().expect("public key invalid");

    assert!(response.verify(&public_key, &challenge));
}

#[test]
fn peer_rejects_wrong_challenge() {
    let (device_key, device) = generate_device(1_000).expect("device generation failed");

    let original = PeerChallenge::generate(1_000).expect("challenge generation failed");

    let different = PeerChallenge::generate(1_000).expect("challenge generation failed");

    let node_id = NodeId::from_device(&device);

    let response = PeerResponse::sign(
        device_key.signing_key(),
        node_id,
        device.device_id,
        &original,
    );

    let public_key = device.public_key().expect("public key invalid");

    assert!(!response.verify(&public_key, &different));
}

#[test]
fn peer_state_transitions() {
    let (_, device) = generate_device(1_000).expect("device generation failed");

    let node_id = NodeId::from_device(&device);

    let mut peer = Peer::discovered(node_id, 1_000);

    assert_eq!(peer.state(), PeerState::Discovered);

    peer.transition(PeerState::Connecting, 1_001);

    peer.transition(PeerState::Challenging, 1_002);

    peer.transition(PeerState::Authenticating, 1_003);

    peer.transition(PeerState::Authenticated, 1_004);

    peer.transition(PeerState::Established, 1_005);

    assert_eq!(peer.state(), PeerState::Established);

    assert_eq!(peer.last_activity(), 1_005);
}

#[test]
fn failed_peer_enters_failed_state() {
    let (_, device) = generate_device(1_000).expect("device generation failed");

    let node_id = NodeId::from_device(&device);

    let mut peer = Peer::discovered(node_id, 1_000);

    peer.fail(1_100);

    assert_eq!(peer.state(), PeerState::Failed);
}
