pub mod bandwidth;
pub mod l4_engine;
pub mod media;
pub mod native;
pub mod udp;
pub mod usage;

pub use bandwidth::{BandwidthLimit, NodeBandwidthPolicy, TrafficBudget, TransferWindow};

pub use l4_engine::{
    AetherL4Session, CongestionState, DEFAULT_INITIAL_CONGESTION_WINDOW,
    DEFAULT_MAX_CONGESTION_WINDOW, DEFAULT_MAX_IN_FLIGHT, DEFAULT_MAX_RECV_BUFFER,
    DEFAULT_MAX_RETRANSMIT_TIMEOUT_MS, DEFAULT_MIN_RETRANSMIT_TIMEOUT_MS,
    DEFAULT_RETRANSMIT_TIMEOUT_MS, L4ReceiveError, L4SendError, L4SessionState,
};

pub use media::{
    CALL_ID_LENGTH, CallSignal, DEFAULT_JITTER_BUFFER_FRAMES, JitterBuffer, JitterPush,
    MAX_MEDIA_PAYLOAD, MEDIA_HEADER_LEN, MEDIA_MAGIC, MEDIA_VERSION, MediaCodec, MediaError,
    MediaFrame, MediaFrameHeader, MediaMux, MediaSession, MediaStats, decode_datagram,
    encode_frame,
};

pub use native::{
    AetherL4Segment, AetherTransportMode, L4_FLAG_ACK, L4_FLAG_FIN, L4_FLAG_RESET, L4_FLAG_SYN,
    L4_HEADER_LENGTH, L4_MAGIC, L4_VERSION, L4Error, MAX_L4_PAYLOAD,
};

pub use udp::{MAX_DATAGRAM_SIZE, UdpTransport, UdpTransportError};

pub use usage::{TransportUsage, TransportUsageTable};
