use aether_core::identity::device::generate_device;
use aether_core::link::{
    LinkSession,
    SessionNegotiation,
    SessionRole,
};
use aether_core::network::{
    NodeId,
    PacketFlags,
    MessageType,
    TrafficPriority,
};

#[test]
fn two_peers_derive_compatible_session_keys() {
    let (_, device_a) =
        generate_device(1000)
            .expect("device A generation failed");

    let (_, device_b) =
        generate_device(1000)
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
        b"AETHER-LINK-TEST",
        node_a.as_bytes(),
        node_b.as_bytes(),
        initiator_public.as_bytes(),
        responder_public.as_bytes(),
    ];

    let mut a =
        initiator
            .finish(
                &responder_public,
                &transcript,
                node_a,
                node_b,
            )
            .expect("initiator session failed");

    let mut b =
        responder
            .finish(
                &initiator_public,
                &transcript,
                node_a,
                node_b,
            )
            .expect("responder session failed");

    let plaintext =
        b"Aether secure transport";

    let frame =
        a.encrypt(
            MessageType::Data,
            TrafficPriority::Messaging,
            PacketFlags::RELAYABLE,
            plaintext,
        )
        .expect("encryption failed");

    let decrypted =
        b.decrypt(&frame)
            .expect("decryption failed");

    assert_eq!(
        decrypted.payload,
        plaintext
    );
}

#[test]
fn encrypted_frame_rejects_tampering() {
    let (_, device_a) =
        generate_device(1000)
            .expect("device A generation failed");

    let (_, device_b) =
        generate_device(1000)
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
        b"AETHER-LINK-TAMPER-TEST",
        node_a.as_bytes(),
        node_b.as_bytes(),
        initiator_public.as_bytes(),
        responder_public.as_bytes(),
    ];

    let mut sender =
        initiator
            .finish(
                &responder_public,
                &transcript,
                node_a,
                node_b,
            )
            .expect("sender session failed");

    let mut receiver =
        responder
            .finish(
                &initiator_public,
                &transcript,
                node_a,
                node_b,
            )
            .expect("receiver session failed");

    let mut frame =
        sender
            .encrypt(
                MessageType::Data,
                TrafficPriority::Messaging,
                PacketFlags::RELAYABLE,
                b"secret",
            )
            .expect("encryption failed");

    let last =
        frame
            .len()
            .checked_sub(1)
            .expect("frame should not be empty");

    frame[last] ^= 0x01;

    assert!(
        receiver.decrypt(&frame).is_err()
    );
}

#[test]
fn replayed_frame_is_rejected() {
    let (_, device_a) =
        generate_device(1000)
            .expect("device A generation failed");

    let (_, device_b) =
        generate_device(1000)
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
        b"AETHER-LINK-REPLAY-TEST",
        node_a.as_bytes(),
        node_b.as_bytes(),
        initiator_public.as_bytes(),
        responder_public.as_bytes(),
    ];

    let mut sender =
        initiator
            .finish(
                &responder_public,
                &transcript,
                node_a,
                node_b,
            )
            .expect("sender session failed");

    let mut receiver =
        responder
            .finish(
                &initiator_public,
                &transcript,
                node_a,
                node_b,
            )
            .expect("receiver session failed");

    let frame =
        sender
            .encrypt(
                MessageType::Data,
                TrafficPriority::Messaging,
                PacketFlags::RELAYABLE,
                b"one-time payload",
            )
            .expect("encryption failed");

    receiver
        .decrypt(&frame)
        .expect("first delivery should succeed");

    assert!(
        receiver.decrypt(&frame).is_err()
    );
}

#[test]
fn session_uses_different_directional_keys() {
    let (_, device_a) =
        generate_device(1000)
            .expect("device A generation failed");

    let (_, device_b) =
        generate_device(1000)
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
        b"AETHER-LINK-DIRECTION-TEST",
        node_a.as_bytes(),
        node_b.as_bytes(),
        initiator_public.as_bytes(),
        responder_public.as_bytes(),
    ];

    let a =
        initiator
            .finish(
                &responder_public,
                &transcript,
                node_a,
                node_b,
            )
            .expect("initiator session failed");

    let b =
        responder
            .finish(
                &initiator_public,
                &transcript,
                node_a,
                node_b,
            )
            .expect("responder session failed");

    assert_eq!(
        a.session_id(),
        b.session_id()
    );
}
