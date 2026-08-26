use crate::network::protocol::TrafficPriority;
use crate::services::service::ServiceKind;

#[derive(Debug, Clone, Copy)]
pub struct ServiceTrafficPolicy;

impl ServiceTrafficPolicy {
    pub fn classify(service: ServiceKind) -> TrafficPriority {
        match service {
            ServiceKind::Core => TrafficPriority::Interactive,

            ServiceKind::Communication => TrafficPriority::Messaging,

            ServiceKind::Storage => TrafficPriority::Background,

            ServiceKind::Compute => TrafficPriority::Interactive,

            ServiceKind::Gaming => TrafficPriority::Interactive,

            ServiceKind::Identity => TrafficPriority::Messaging,

            ServiceKind::Infrastructure => TrafficPriority::Interactive,

            ServiceKind::ThirdParty => TrafficPriority::Browsing,
        }
    }
}
