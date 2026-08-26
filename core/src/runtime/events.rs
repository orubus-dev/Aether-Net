use crate::network::node::NodeId;

#[derive(Debug, Clone)]
pub enum CoreEvent {
    NodeDiscovered { node_id: NodeId },

    NodeDisconnected { node_id: NodeId },

    SessionEstablished { node_id: NodeId },

    SessionClosed { node_id: NodeId },

    PacketDropped,

    RouteChanged,

    GatewayChanged,
}
