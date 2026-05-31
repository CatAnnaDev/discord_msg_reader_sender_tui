use std::error::Error;

use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde_json::json;

use crate::utils::{http_client, SnowflakeID};

const API_BASE: &str = "https://discord.com/api/v9";

fn auth_headers(token: &str) -> Result<HeaderMap, Box<dyn Error>> {
    let mut h = HeaderMap::new();
    h.insert(AUTHORIZATION, HeaderValue::from_str(token)?);
    h.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    Ok(h)
}

pub async fn fetch_messages(
    token: &str,
    channel_id: SnowflakeID,
    limit: u16,
) -> Result<Vec<serde_json::Value>, Box<dyn Error>> {
    let url = format!("{API_BASE}/channels/{channel_id}/messages?limit={limit}");
    let resp = http_client()
        .get(&url)
        .headers(auth_headers(token)?)
        .send()
        .await?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("GET {url} returned {status}: {body}").into());
    }
    Ok(resp.json::<Vec<serde_json::Value>>().await?)
}

pub async fn delete_message(
    token: &str,
    channel_id: SnowflakeID,
    message_id: SnowflakeID,
) -> Result<(), Box<dyn Error>> {
    let url = format!("{API_BASE}/channels/{channel_id}/messages/{message_id}");
    let resp = http_client()
        .delete(&url)
        .headers(auth_headers(token)?)
        .send()
        .await?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("DELETE {url} returned {status}: {body}").into());
    }
    Ok(())
}

pub async fn edit_message(
    token: &str,
    channel_id: SnowflakeID,
    message_id: SnowflakeID,
    content: &str,
) -> Result<(), Box<dyn Error>> {
    let url = format!("{API_BASE}/channels/{channel_id}/messages/{message_id}");
    let body = json!({ "content": content });
    let resp = http_client()
        .patch(&url)
        .headers(auth_headers(token)?)
        .json(&body)
        .send()
        .await?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("PATCH {url} returned {status}: {body}").into());
    }
    Ok(())
}

pub async fn send_message(
    token: &str,
    channel_id: SnowflakeID,
    content: &str,
    nonce: u64,
    reply_to: Option<SnowflakeID>,
) -> Result<(), Box<dyn Error>> {
    let url = format!("{API_BASE}/channels/{channel_id}/messages");
    let mut body = json!({
        "content": content,
        "nonce": nonce.to_string(),
        "tts": false,
        "flags": 0,
    });
    if let Some(mid) = reply_to {
        body["message_reference"] = json!({
            "channel_id": channel_id.to_string(),
            "message_id": mid.to_string(),
        });
    }
    let resp = http_client()
        .post(&url)
        .headers(auth_headers(token)?)
        .json(&body)
        .send()
        .await?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("POST {url} returned {status}: {body}").into());
    }
    Ok(())
}

pub async fn ack_message(
    token: &str,
    channel_id: SnowflakeID,
    message_id: SnowflakeID,
) -> Result<(), Box<dyn Error>> {
    let url = format!("{API_BASE}/channels/{channel_id}/messages/{message_id}/ack");
    let resp = http_client()
        .post(&url)
        .headers(auth_headers(token)?)
        .json(&json!({ "token": serde_json::Value::Null }))
        .send()
        .await?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("POST {url} returned {status}: {body}").into());
    }
    Ok(())
}

pub async fn upload_file(
    token: &str,
    channel_id: SnowflakeID,
    path: &str,
    nonce: u64,
) -> Result<(), Box<dyn Error>> {
    let bytes = std::fs::read(path)?;
    let filename = std::path::Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("file")
        .to_string();
    let payload = json!({
        "content": "",
        "nonce": nonce.to_string(),
        "tts": false,
        "attachments": [{ "id": 0, "filename": filename }],
    });
    let part = reqwest::multipart::Part::bytes(bytes).file_name(filename.clone());
    let form = reqwest::multipart::Form::new()
        .text("payload_json", payload.to_string())
        .part("files[0]", part);
    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, HeaderValue::from_str(token)?);
    let url = format!("{API_BASE}/channels/{channel_id}/messages");
    let resp = http_client()
        .post(&url)
        .headers(headers)
        .multipart(form)
        .send()
        .await?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("UPLOAD {url} returned {status}: {body}").into());
    }
    Ok(())
}

pub async fn accept_invite(
    token: &str,
    code: &str,
) -> Result<serde_json::Value, Box<dyn Error>> {
    let code = code
        .trim()
        .rsplit('/')
        .next()
        .unwrap_or(code)
        .split('?')
        .next()
        .unwrap_or(code)
        .trim();
    let url = format!("{API_BASE}/invites/{code}");
    let resp = http_client()
        .post(&url)
        .headers(auth_headers(token)?)
        .json(&json!({}))
        .send()
        .await?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("POST {url} returned {status}: {body}").into());
    }
    Ok(resp.json::<serde_json::Value>().await?)
}

pub async fn leave_guild(
    token: &str,
    guild_id: SnowflakeID,
) -> Result<(), Box<dyn Error>> {
    let url = format!("{API_BASE}/users/@me/guilds/{guild_id}");
    let resp = http_client()
        .delete(&url)
        .headers(auth_headers(token)?)
        .json(&json!({ "lurking": false }))
        .send()
        .await?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("DELETE {url} returned {status}: {body}").into());
    }
    Ok(())
}

pub async fn create_dm(
    token: &str,
    recipient_id: SnowflakeID,
) -> Result<serde_json::Value, Box<dyn Error>> {
    let url = format!("{API_BASE}/users/@me/channels");
    let resp = http_client()
        .post(&url)
        .headers(auth_headers(token)?)
        .json(&json!({ "recipients": [recipient_id.to_string()] }))
        .send()
        .await?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("POST {url} returned {status}: {body}").into());
    }
    Ok(resp.json::<serde_json::Value>().await?)
}
