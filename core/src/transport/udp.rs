use std::net::SocketAddr;
use tokio::net::UdpSocket;

pub const MAX_DATAGRAM_SIZE: usize = 65_535;

pub struct UdpTransport {
    socket: UdpSocket,
}

impl UdpTransport {
    pub async fn bind(address: SocketAddr) -> Result<Self, UdpTransportError> {
        let socket = UdpSocket::bind(address)
            .await
            .map_err(UdpTransportError::Bind)?;

        Ok(Self { socket })
    }

    pub fn local_addr(&self) -> Result<SocketAddr, UdpTransportError> {
        self.socket
            .local_addr()
            .map_err(UdpTransportError::LocalAddress)
    }

    pub async fn send_to(
        &self,
        data: &[u8],
        destination: SocketAddr,
    ) -> Result<usize, UdpTransportError> {
        if data.len() > MAX_DATAGRAM_SIZE {
            return Err(UdpTransportError::DatagramTooLarge);
        }

        self.socket
            .send_to(data, destination)
            .await
            .map_err(UdpTransportError::Send)
    }

    pub async fn receive(&self) -> Result<(Vec<u8>, SocketAddr), UdpTransportError> {
        let mut buffer = vec![0u8; MAX_DATAGRAM_SIZE];

        let (length, source) = self
            .socket
            .recv_from(&mut buffer)
            .await
            .map_err(UdpTransportError::Receive)?;

        buffer.truncate(length);

        Ok((buffer, source))
    }

    pub async fn receive_into(
        &self,
        buffer: &mut [u8],
    ) -> Result<(usize, SocketAddr), UdpTransportError> {
        self.socket
            .recv_from(buffer)
            .await
            .map_err(UdpTransportError::Receive)
    }
}

#[derive(Debug)]
pub enum UdpTransportError {
    Bind(std::io::Error),
    LocalAddress(std::io::Error),
    Send(std::io::Error),
    Receive(std::io::Error),
    DatagramTooLarge,
}
