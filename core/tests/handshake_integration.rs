use aether_core::identity::device::generate_device;
use aether_core::network::NodeId;
use aether_core::runtime::peers::{AuthenticationState, HandshakeManager};
use std::time::Instant;

#[test]
fn two_devices_complete_authenticated_handshake() {
    let (_initiator_key, initiator_device) =
        generate_device(1).expect("initiator generation failed");

    let (responder_key, responder_device) =
        generate_device(1).expect("responder generation failed");

    let initiator_node = NodeId::from_device(&initiator_device);

    let responder_node = NodeId::from_device(&responder_device);

    let now = Instant::now();

    let mut initiator = HandshakeManager::new();

    let mut responder = HandshakeManager::new();

    let challenge = initiator
        .start_initiator(initiator_node, responder_node, now)
        .expect("initiator challenge failed");

    let (response, responder_session) = responder
        .respond(&challenge, &responder_device, &responder_key, now)
        .expect("responder handshake failed");

    let initiator_session = initiator
        .complete_initiator(
            &response,
            &responder_device,
            initiator_node,
            responder_node,
            now,
        )
        .expect("initiator completion failed");

    assert_eq!(
        initiator_session.session_id(),
        responder_session.session_id(),
    );

    assert!(initiator.is_established(&responder_node));

    assert_eq!(
        initiator.authentication(&responder_node).unwrap().state,
        AuthenticationState::Established,
    );
}

#[test]
fn handshake_rejects_tampered_response() {
    let (_initiator_key, initiator_device) = generate_device(1).expect("initiator failed");

    let (responder_key, responder_device) = generate_device(1).expect("responder failed");

    let initiator_node = NodeId::from_device(&initiator_device);

    let responder_node = NodeId::from_device(&responder_device);

    let now = Instant::now();

    let mut initiator = HandshakeManager::new();

    let mut responder = HandshakeManager::new();

    let challenge = initiator
        .start_initiator(initiator_node, responder_node, now)
        .expect("challenge failed");

    let (mut response, _) = responder
        .respond(&challenge, &responder_device, &responder_key, now)
        .expect("response failed");

    response.timestamp = response.timestamp.saturating_add(1);

    let result = initiator.complete_initiator(
        &response,
        &responder_device,
        initiator_node,
        responder_node,
        now,
    );

    assert!(result.is_err());
}
#[test]
fn handshake_rejects_wrong_device_identity() {
    let (_initiator_key, initiator_device) = generate_device(1).expect("initiator failed");

    let (responder_key, responder_device) = generate_device(1).expect("responder failed");

    let (wrong_key, wrong_device) = generate_device(1).expect("wrong device failed");

    let initiator_node = NodeId::from_device(&initiator_device);

    let responder_node = NodeId::from_device(&responder_device);

    let now = Instant::now();

    let mut initiator = HandshakeManager::new();

    let mut responder = HandshakeManager::new();

    let challenge = initiator
        .start_initiator(initiator_node, responder_node, now)
        .expect("challenge failed");

    let (response, _) = responder
        .respond(&challenge, &wrong_device, &wrong_key, now)
        .expect("response creation failed");

    let result = initiator.complete_initiator(
        &response,
        &responder_device,
        initiator_node,
        responder_node,
        now,
    );

    assert!(result.is_err());

    let _ = _initiator_key;

    let _ = responder_key;
}
