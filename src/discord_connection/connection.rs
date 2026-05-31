use std::error::Error;

use futures_util::SinkExt;
use futures_util::stream::SplitSink;
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};
use tungstenite::Message;

use crate::send_struct::resume::{DiscordResumeConnection, DiscordResumeData};
use crate::{error, info};

pub struct ConnectDiscord {
    pub socket: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

impl ConnectDiscord {
    pub async fn new(url: &str) -> Result<Self, Box<dyn Error>> {
        let (socket, _) = connect_async(url).await?;
        Ok(Self { socket })
    }

    pub async fn resume(
        socket: &mut SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
        token: &str,
        s_id: &str,
        seq: u64,
    ) -> Result<(), Box<dyn Error>> {
        let json_resume = DiscordResumeConnection {
            op: 6,
            d: DiscordResumeData {
                token: token.to_string(),
                session_id: s_id.to_string(),
                seq,
            },
        };
        let body = serde_json::to_string(&json_resume)?;
        match socket.send(Message::text(body)).await {
            Ok(_) => {
                info!("Connection Resume");
                Ok(())
            }
            Err(e) => {
                error!("Connection Resume: {e}");
                Err(Box::new(e))
            }
        }
    }
}
