use std::error::Error;
use std::net::SocketAddr;

use byteorder::{BigEndian, ByteOrder};
use tokio::net::UdpSocket;

pub struct DiscoveredAddress {
    pub address: String,
    pub port: u16,
}

pub async fn ip_discovery(
    sock: &UdpSocket,
    ssrc: u32,
    target: SocketAddr,
) -> Result<DiscoveredAddress, Box<dyn Error + Send + Sync>> {
    let mut packet = [0u8; 74];

    BigEndian::write_u16(&mut packet[0..2], 0x0001);
    BigEndian::write_u16(&mut packet[2..4], 70);
    BigEndian::write_u32(&mut packet[4..8], ssrc);
    sock.send_to(&packet, target).await?;

    let mut buf = [0u8; 74];
    loop {
        let (n, peer) = sock.recv_from(&mut buf).await?;
        if peer != target {
            continue;
        }
        if n < 74 {
            return Err(format!("discovery reply too short: {n}").into());
        }

        let address_end = buf[8..72]
            .iter()
            .position(|&b| b == 0)
            .map(|p| 8 + p)
            .unwrap_or(72);
        let address = std::str::from_utf8(&buf[8..address_end])?.to_string();
        let port = BigEndian::read_u16(&buf[72..74]);
        return Ok(DiscoveredAddress { address, port });
    }
}

pub fn make_rtp_header(seq: u16, timestamp: u32, ssrc: u32) -> [u8; 12] {
    let mut h = [0u8; 12];
    h[0] = 0x80;
    h[1] = 0x78;
    BigEndian::write_u16(&mut h[2..4], seq);
    BigEndian::write_u32(&mut h[4..8], timestamp);
    BigEndian::write_u32(&mut h[8..12], ssrc);
    h
}
