use std::error::Error;

use futures_util::SinkExt;

use crate::discord_connection::GatewaySink;
use crate::error;
use tungstenite::Message;

pub async fn heart_beat(
    socket: &mut GatewaySink,
    last_s: Option<u64>,
) -> Result<(), Box<dyn Error>> {
    let body = match last_s {
        Some(s) => format!(r#"{{"op": 1, "d": {}}}"#, s),
        None => r#"{"op": 1, "d": null}"#.to_string(),
    };
    if let Err(err) = socket.send(Message::text(body)).await {
        error!("Error sending heartbeat: {:?}", err);
        return Err(Box::new(err));
    }
    Ok(())
}
