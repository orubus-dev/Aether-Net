use crate::network::protocol::MessageType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseReason {
    Normal = 0,
    Timeout = 1,
    ProtocolError = 2,
    AuthenticationFailure = 3,
    KeyRotation = 4,
    ResourceLimit = 5,
    Administrative = 6,
}

impl CloseReason {
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Normal),
            1 => Some(Self::Timeout),
            2 => Some(Self::ProtocolError),
            3 => Some(Self::AuthenticationFailure),
            4 => Some(Self::KeyRotation),
            5 => Some(Self::ResourceLimit),
            6 => Some(Self::Administrative),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlMessage {
    KeepAlive {
        timestamp: u64,
    },
    KeepAliveAck {
        timestamp: u64,
    },
    Rekey {
        epoch: u64,
    },
    RekeyAck {
        epoch: u64,
    },
    Close {
        reason: CloseReason,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlError {
    WrongMessageType,
    InvalidLength,
    UnknownCloseReason,
}

impl ControlMessage {
    pub fn message_type(&self) -> MessageType {
        match self {
            Self::KeepAlive { .. } =>
                MessageType::KeepAlive,

            Self::KeepAliveAck { .. } =>
                MessageType::Acknowledgement,

            Self::Rekey { .. } =>
                MessageType::Rekey,

            Self::RekeyAck { .. } =>
                MessageType::RekeyAck,

            Self::Close { .. } =>
                MessageType::Disconnect,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        match self {
            Self::KeepAlive {
                timestamp,
            } => {
                let mut bytes =
                    vec![1u8];

                bytes.extend_from_slice(
                    &timestamp.to_be_bytes(),
                );

                bytes
            }

            Self::KeepAliveAck {
                timestamp,
            } => {
                let mut bytes =
                    vec![2u8];

                bytes.extend_from_slice(
                    &timestamp.to_be_bytes(),
                );

                bytes
            }

            Self::Rekey { epoch } => {
                let mut bytes =
                    vec![3u8];

                bytes.extend_from_slice(
                    &epoch.to_be_bytes(),
                );

                bytes
            }

            Self::RekeyAck { epoch } => {
                let mut bytes =
                    vec![4u8];

                bytes.extend_from_slice(
                    &epoch.to_be_bytes(),
                );

                bytes
            }

            Self::Close { reason } =>
                vec![
                    5u8,
                    *reason as u8,
                ],
        }
    }

    pub fn decode(
        message_type: MessageType,
        bytes: &[u8],
    ) -> Result<Self, ControlError> {
        if bytes.is_empty() {
            return Err(
                ControlError::InvalidLength
            );
        }

        match bytes[0] {
            1 if message_type
                == MessageType::KeepAlive => {

                if bytes.len() != 9 {
                    return Err(
                        ControlError::InvalidLength
                    );
                }

                let timestamp =
                    u64::from_be_bytes(
                        bytes[1..9]
                            .try_into()
                            .map_err(
                                |_| ControlError::InvalidLength
                            )?,
                    );

                Ok(Self::KeepAlive {
                    timestamp,
                })
            }

            2 if message_type
                == MessageType::Acknowledgement => {

                if bytes.len() != 9 {
                    return Err(
                        ControlError::InvalidLength
                    );
                }

                let timestamp =
                    u64::from_be_bytes(
                        bytes[1..9]
                            .try_into()
                            .map_err(
                                |_| ControlError::InvalidLength
                            )?,
                    );

                Ok(Self::KeepAliveAck {
                    timestamp,
                })
            }

            3 if message_type
                == MessageType::Rekey => {

                if bytes.len() != 9 {
                    return Err(
                        ControlError::InvalidLength
                    );
                }

                let epoch =
                    u64::from_be_bytes(
                        bytes[1..9]
                            .try_into()
                            .map_err(
                                |_| ControlError::InvalidLength
                            )?,
                    );

                Ok(Self::Rekey { epoch })
            }

            4 if message_type
                == MessageType::RekeyAck => {

                if bytes.len() != 9 {
                    return Err(
                        ControlError::InvalidLength
                    );
                }

                let epoch =
                    u64::from_be_bytes(
                        bytes[1..9]
                            .try_into()
                            .map_err(
                                |_| ControlError::InvalidLength
                            )?,
                    );

                Ok(Self::RekeyAck { epoch })
            }

            5 if message_type
                == MessageType::Disconnect => {

                if bytes.len() != 2 {
                    return Err(
                        ControlError::InvalidLength
                    );
                }

                let reason =
                    CloseReason::from_u8(
                        bytes[1],
                    )
                    .ok_or(
                        ControlError::UnknownCloseReason
                    )?;

                Ok(Self::Close { reason })
            }

            _ =>
                Err(
                    ControlError::WrongMessageType
                ),
        }
    }
}
