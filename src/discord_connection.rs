use std::error::Error;

use futures_util::StreamExt;
use futures_util::stream::{SplitSink, SplitStream};
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use tungstenite::Message;

use crate::discord_connection::connection::ConnectDiscord;

pub mod connection;

pub type GatewaySink = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;
pub type GatewayStream = SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>;

pub async fn discord_wss_connection(
    url: &str,
) -> Result<(GatewaySink, GatewayStream), Box<dyn Error>> {
    Ok(ConnectDiscord::new(url).await?.socket.split())
}

pub async fn discord_wss_resume(
    socket: &mut GatewaySink,
    token: &str,
    session_id: &str,
    seq: u64,
) -> Result<(), Box<dyn Error>> {
    ConnectDiscord::resume(socket, token, session_id, seq).await
}
