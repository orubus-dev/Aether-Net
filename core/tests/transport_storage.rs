use aether_core::storage::{ChunkId, ChunkRequest, ContentId, RequestDeduplicator};
use aether_core::transport::{BandwidthLimit, TransferWindow};
use std::time::Instant;

#[test]
fn content_id_is_deterministic() {
    let first = ContentId::from_bytes(b"aether-content");

    let second = ContentId::from_bytes(b"aether-content");

    assert_eq!(first, second);
}

#[test]
fn different_content_has_different_identifier() {
    let first = ContentId::from_bytes(b"content-a");

    let second = ContentId::from_bytes(b"content-b");

    assert_ne!(first, second);
}

#[test]
fn duplicate_chunk_requests_are_suppressed() {
    let content = ContentId::from_bytes(b"large-file");

    let request = ChunkRequest {
        content,
        chunk: ChunkId(10),
    };

    let mut dedupe = RequestDeduplicator::new();

    assert!(dedupe.request(request));

    assert!(!dedupe.request(request));

    assert_eq!(dedupe.pending_count(), 1);

    assert!(dedupe.complete(&request));

    assert_eq!(dedupe.pending_count(), 0);
}

#[test]
fn bandwidth_limit_converts_bits_to_bytes() {
    let limit = BandwidthLimit::new(80_000_000);

    assert_eq!(limit.bytes_per_second(), 10_000_000);
}

#[test]
fn transfer_window_tracks_rates() {
    let start = Instant::now();

    let mut window = TransferWindow::new(start);

    window.record_sent(1_000);

    window.record_received(2_000);

    let now = start + std::time::Duration::from_secs(1);

    assert_eq!(window.bytes_sent, 1_000);

    assert_eq!(window.bytes_received, 2_000);

    assert_eq!(window.send_rate_bps(now), 8_000);

    assert_eq!(window.receive_rate_bps(now), 16_000);
}
