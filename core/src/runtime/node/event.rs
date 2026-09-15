use crate::discovery::FormationState;
use crate::network::NodeId;
use crate::runtime::discovery::DiscoveryHello;
use std::net::SocketAddr;

#[derive(Debug)]
pub enum NodeEvent {
    Started,
    ReceivedHello {
        hello: DiscoveryHello,
        address: SocketAddr,
    },
    PeerDiscovered {
        node_id: NodeId,
        address: SocketAddr,
    },
    PeerExpired {
        node_id: NodeId,
    },
    FormationChanged {
        state: FormationState,
        generation: u64,
    },
    Tick,
    Shutdown,
}
