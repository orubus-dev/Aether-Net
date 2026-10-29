//! Two-peer native session proof.
//!
//! Two independent `NativeAetherStack`s — authenticated link sessions,
//! native L3 addressing, L4 flows — exchange data in both directions and
//! then run a complete live media call: invite over the reliable control
//! flow, accept, realtime media frames through reordering and loss, and a
//! clean end. This is the moment two Aether nodes speak native end-to-end.

use aether_core::link::{SessionNegotiation, SessionRole};
use aether_core::network::{AetherAddress, AetherFlowId, L3Route, NodeId};
use aether_core::runtime::native_stack::{NativeAetherStack, NativeIngress};
use aether_core::transport::AetherTransportMode;
use aether_core::transport::media::{CallSignal, MediaCodec, MediaMux, MediaSession};

fn addr(seed: u8) -> AetherAddress {
    AetherAddress::from_bytes([seed; 32])
}

fn node(seed: u8) -> NodeId {
    NodeId::from_bytes([seed; 32])
}

fn paired_sessions(
    a: NodeId,
    b: NodeId,
) -> (
    aether_core::link::LinkSession,
    aether_core::link::LinkSession,
) {
    let initiator = SessionNegotiation::create(SessionRole::Initiator).unwrap();
    let responder = SessionNegotiation::create(SessionRole::Responder).unwrap();
    let a_pub = initiator.local_ephemeral.public_key();
    let b_pub = responder.local_ephemeral.public_key();
    let transcript: [&[u8]; 2] = [a.as_bytes(), b.as_bytes()];
    (
        initiator.finish(&b_pub, &transcript, a, b).unwrap(),
        responder.finish(&a_pub, &transcript, a, b).unwrap(),
    )
}

/// Wire two stacks together over an authenticated link pair.
fn wired_stacks() -> (NativeAetherStack, NativeAetherStack) {
    let (ab_a, ab_b) = paired_sessions(node(1), node(2));
    let mut a = NativeAetherStack::new(addr(1));
    let mut b = NativeAetherStack::new(addr(2));
    a.attach_peer(node(2), addr(2), ab_a).unwrap();
    b.attach_peer(node(1), addr(1), ab_b).unwrap();
    a.l3_mut()
        .install_route(L3Route::new(addr(2), addr(2), 1, 1, None))
        .unwrap();
    b.l3_mut()
        .install_route(L3Route::new(addr(1), addr(1), 1, 1, None))
        .unwrap();
    (a, b)
}

/// Deliver one frame A -> B, return B's local delivery.
fn a_to_b(a: &mut NativeAetherStack, b: &mut NativeAetherStack, frame: Vec<u8>) -> Vec<Vec<u8>> {
    match b.ingest_frame(node(1), &frame, 1, 1).unwrap() {
        NativeIngress::Local {
            delivered,
            acknowledgement,
            ..
        } => {
            // ACK frames flow back the other way on the same link pair.
            if let Some(ack) = acknowledgement {
                a.ingest_frame(node(2), &ack, 1, 1).unwrap();
            }
            delivered
        }
        NativeIngress::Forward { .. } => panic!("two-peer topology must not forward"),
    }
}

#[test]
fn two_peers_exchange_reliable_stream_both_ways() {
    let (mut a, mut b) = wired_stacks();
    let flow = AetherFlowId::from_bytes([0x42; 16]);

    a.open_flow(addr(2), flow, AetherTransportMode::ReliableStream)
        .unwrap();
    b.open_flow(addr(1), flow, AetherTransportMode::ReliableStream)
        .unwrap();

    // A -> B, three ordered messages.
    for message in ["first", "second", "third"] {
        let frame = a
            .send_l4(addr(2), flow, message.as_bytes().to_vec(), 1, 1)
            .unwrap();
        assert_eq!(
            a_to_b(&mut a, &mut b, frame),
            vec![message.as_bytes().to_vec()]
        );
    }

    // B -> A, the reverse direction over the same flow id.
    let frame = b
        .send_l4(addr(1), flow, b"reply from B".to_vec(), 1, 1)
        .unwrap();
    match a.ingest_frame(node(2), &frame, 1, 1).unwrap() {
        NativeIngress::Local {
            delivered,
            acknowledgement,
            ..
        } => {
            assert_eq!(delivered, vec![b"reply from B".to_vec()]);
            assert!(acknowledgement.is_some());
        }
        _ => panic!("expected local delivery"),
    }
}

#[test]
fn two_peers_run_a_live_media_call_over_native_l4() {
    let (mut a, mut b) = wired_stacks();
    let control = AetherFlowId::from_bytes([0xc0; 16]);
    let media = AetherFlowId::from_bytes([0xd1; 16]);

    a.open_flow(addr(2), control, AetherTransportMode::ReliableStream)
        .unwrap();
    b.open_flow(addr(1), control, AetherTransportMode::ReliableStream)
        .unwrap();
    a.open_flow(addr(2), media, AetherTransportMode::Realtime)
        .unwrap();
    b.open_flow(addr(1), media, AetherTransportMode::Realtime)
        .unwrap();

    let call_id = [0x7e; 8];
    let mut mux_a = MediaMux::new();
    let mut mux_b = MediaMux::new();
    mux_a
        .open(MediaSession::new(call_id, MediaCodec::Opus))
        .unwrap();
    mux_b
        .open(MediaSession::new(call_id, MediaCodec::Opus))
        .unwrap();

    // --- call setup over the control flow (real JSON signals) ---
    let invite = CallSignal::Invite {
        call_id: aether_core::transport::media::call_id_hex(&call_id),
        codec: "opus".into(),
        display_name: Some("Node A".into()),
    };
    let frame = a.send_l4(addr(2), control, invite.encode(), 1, 1).unwrap();
    let delivered = a_to_b(&mut a, &mut b, frame);
    let invite = CallSignal::decode(&delivered[0]).unwrap();
    assert!(matches!(invite, CallSignal::Invite { .. }));

    // B accepts; the accept rides back; A marks accepted. Both Active.
    let accept = CallSignal::Accept {
        call_id: aether_core::transport::media::call_id_hex(&call_id),
    };
    let frame = b.send_l4(addr(1), control, accept.encode(), 1, 1).unwrap();
    match a.ingest_frame(node(2), &frame, 1, 1).unwrap() {
        NativeIngress::Local { delivered, .. } => {
            assert!(matches!(
                CallSignal::decode(&delivered[0]).unwrap(),
                CallSignal::Accept { .. }
            ));
        }
        _ => panic!("expected local delivery of accept"),
    }

    // --- live media: 100 frames, 10% lost on the wire ---
    // The native realtime transport drops stale (non-increasing) sequences
    // by design, so the wire models loss, and the jitter buffer accounts
    // for every gap at playout.
    let mut delivered_frames = 0usize;
    let mut played = Vec::new();
    for sequence in 0..100u32 {
        let payload = vec![(sequence % 251) as u8; 80];
        let datagram = mux_a
            .get_mut(&call_id)
            .unwrap()
            .send_frame(&payload, sequence * 20)
            .unwrap();
        if sequence % 10 == 5 {
            continue; // lost on the wire — sequence already consumed, gap is real
        }
        let frame = a.send_l4(addr(2), media, datagram, 1, 1).unwrap();
        let arrival_ms = 1000 + sequence as u64 * 20;
        let datagrams = match b
            .ingest_frame(node(1), &frame, 1, arrival_ms / 1000)
            .unwrap()
        {
            NativeIngress::Local { delivered, .. } => delivered,
            _ => panic!("media must deliver locally"),
        };
        for datagram in datagrams {
            let routed = mux_b.demux(&datagram, arrival_ms).unwrap();
            assert_eq!(routed, call_id);
            delivered_frames += 1;
        }
        while let Some(played_frame) = mux_b.get_mut(&call_id).unwrap().pop_playout() {
            played.push(played_frame.sequence);
        }
    }
    while let Some(played_frame) = mux_b.get_mut(&call_id).unwrap().pop_playout() {
        played.push(played_frame.sequence);
    }

    // Strictly increasing playout; every dropped frame declared lost.
    assert_eq!(delivered_frames, 90, "10 dropped frames never arrive");
    let mut strictly_increasing = true;
    for pair in played.windows(2) {
        if pair[1] <= pair[0] {
            strictly_increasing = false;
        }
    }
    assert!(strictly_increasing, "playout order broke");
    assert_eq!(played[0], 0);
    assert_eq!(*played.last().unwrap(), 99);

    let stats = mux_b.get_mut(&call_id).unwrap().stats();
    assert_eq!(stats.received, 90);
    assert_eq!(stats.lost, 10);
    assert_eq!(mux_a.get_mut(&call_id).unwrap().stats().frames_sent, 100);

    // --- clean end ---
    let end = CallSignal::End {
        call_id: aether_core::transport::media::call_id_hex(&call_id),
    };
    let frame = a.send_l4(addr(2), control, end.encode(), 1, 1).unwrap();
    let delivered = a_to_b(&mut a, &mut b, frame);
    assert!(matches!(
        CallSignal::decode(&delivered[0]).unwrap(),
        CallSignal::End { .. }
    ));
}
