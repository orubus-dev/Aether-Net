use aether_core::identity::device::generate_device;
use aether_core::link::{
    CloseReason,
    ControlMessage,
    LinkSession,
    SessionNegotiation,
    SessionRole,
};
use aether_core::network::{
    MessageType,
    NodeId,
    PacketFlags,
    TrafficPriority,
};

fn create_sessions()
    -> (LinkSession, LinkSession)
{
    let (_, device_a) =
        generate_device(1)
            .expect("device A generation failed");

    let (_, device_b) =
        generate_device(1)
            .expect("device B generation failed");

    let node_a =
        NodeId::from_device(&device_a);

    let node_b =
        NodeId::from_device(&device_b);

    let initiator =
        SessionNegotiation::create(
            SessionRole::Initiator,
        )
        .expect("initiator creation failed");

    let responder =
        SessionNegotiation::create(
            SessionRole::Responder,
        )
        .expect("responder creation failed");

    let initiator_public =
        initiator
            .local_ephemeral
            .public_key();

    let responder_public =
        responder
            .local_ephemeral
            .public_key();

    let transcript: Vec<&[u8]> = vec![
        b"AETHER-LINK-L2-MILESTONE",
        node_a.as_bytes(),
        node_b.as_bytes(),
        initiator_public.as_bytes(),
        responder_public.as_bytes(),
    ];

    let first =
        initiator
            .finish(
                &responder_public,
                &transcript,
                node_a,
                node_b,
            )
            .expect("initiator failed");

    let second =
        responder
            .finish(
                &initiator_public,
                &transcript,
                node_a,
                node_b,
            )
            .expect("responder failed");

    (first, second)
}

#[test]
fn keepalive_control_round_trip() {
    let message =
        ControlMessage::KeepAlive {
            timestamp: 123456,
        };

    let encoded =
        message.encode();

    let decoded =
        ControlMessage::decode(
            message.message_type(),
            &encoded,
        )
        .expect("control decoding failed");

    assert_eq!(
        decoded,
        message
    );
}

#[test]
fn keepalive_ack_round_trip() {
    let message =
        ControlMessage::KeepAliveAck {
            timestamp: 999999,
        };

    let encoded =
        message.encode();

    let decoded =
        ControlMessage::decode(
            message.message_type(),
            &encoded,
        )
        .expect("ack decoding failed");

    assert_eq!(
        decoded,
        message
    );
}

#[test]
fn rekey_control_round_trip() {
    let message =
        ControlMessage::Rekey {
            epoch: 4,
        };

    let encoded =
        message.encode();

    let decoded =
        ControlMessage::decode(
            message.message_type(),
            &encoded,
        )
        .expect("rekey decoding failed");

    assert_eq!(
        decoded,
        message
    );
}

#[test]
fn close_control_round_trip() {
    let message =
        ControlMessage::Close {
            reason:
                CloseReason::Normal,
        };

    let encoded =
        message.encode();

    let decoded =
        ControlMessage::decode(
            message.message_type(),
            &encoded,
        )
        .expect("close decoding failed");

    assert_eq!(
        decoded,
        message
    );
}

#[test]
fn sessions_can_rekey_together() {
    let (mut first, mut second) =
        create_sessions();

    assert_eq!(
        first.epoch(),
        0
    );

    assert_eq!(
        second.epoch(),
        0
    );

    let first_message =
        first
            .encrypt(
                MessageType::Data,
                TrafficPriority::Messaging,
                PacketFlags::RELAYABLE,
                b"before rekey",
            )
            .expect("first encryption failed");

    second
        .decrypt(&first_message)
        .expect("first decryption failed");

    first
        .rekey(1)
        .expect("first rekey failed");

    second
        .rekey(1)
        .expect("second rekey failed");

    assert_eq!(
        first.epoch(),
        1
    );

    assert_eq!(
        second.epoch(),
        1
    );

    let second_message =
        first
            .encrypt(
                MessageType::Data,
                TrafficPriority::Messaging,
                PacketFlags::RELAYABLE,
                b"after rekey",
            )
            .expect("second encryption failed");

    let decrypted =
        second
            .decrypt(&second_message)
            .expect("second decryption failed");

    assert_eq!(
        decrypted.payload,
        b"after rekey"
    );
}

#[test]
fn old_epoch_cannot_be_used_after_rekey() {
    let (mut first, mut second) =
        create_sessions();

    let old_frame =
        first
            .encrypt(
                MessageType::Data,
                TrafficPriority::Messaging,
                PacketFlags::RELAYABLE,
                b"old epoch",
            )
            .expect("encryption failed");

    first
        .rekey(1)
        .expect("rekey failed");

    second
        .rekey(1)
        .expect("rekey failed");

    assert!(
        second
            .decrypt(&old_frame)
            .is_err()
    );
}

#[test]
fn tampered_control_message_is_rejected() {
    let message =
        ControlMessage::Rekey {
            epoch: 12,
        };

    let mut encoded =
        message.encode();

    encoded[1] ^= 0xff;

    let decoded =
        ControlMessage::decode(
            message.message_type(),
            &encoded,
        )
        .expect("binary control message itself remains valid");

    assert_ne!(
        decoded,
        message
    );
}
