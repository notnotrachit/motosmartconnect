//! Pull a file the phone is already offering.
//!
//! On the local network the phone serves one transfer as HTTPS plus a short WebSocket
//! conversation, then a zip from `GET /download`. The certificate is pinned to the
//! fingerprint from the control channel. Behavior follows the public Smart Connect
//! notes in smartconnect-linux (MIT).

use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, SignatureScheme};
use sha2::{Digest, Sha256};
use std::net::IpAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::client_async;

const MAX_ZIP: usize = 512 * 1024 * 1024;

#[derive(Debug)]
struct PinCert {
    fingerprint: [u8; 32],
}

impl ServerCertVerifier for PinCert {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let digest = Sha256::digest(end_entity.as_ref());
        if digest.as_slice() == self.fingerprint {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::General("certificate fingerprint does not match".into()))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        let algorithms = rustls::crypto::ring::default_provider().signature_verification_algorithms;
        rustls::crypto::verify_tls12_signature(message, cert, dss, &algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        let algorithms = rustls::crypto::ring::default_provider().signature_verification_algorithms;
        rustls::crypto::verify_tls13_signature(message, cert, dss, &algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

pub fn fingerprint_bytes(text: &str) -> Option<[u8; 32]> {
    let bytes = hex::decode(text.trim()).ok()?;
    if bytes.len() != 32 {
        return None;
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Some(out)
}

fn pinned_config(fingerprint: [u8; 32]) -> ClientConfig {
    let mut config = ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(PinCert { fingerprint }))
        .with_no_client_auth();
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    config
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame<'a> {
    pub kind: &'a str,
    pub id: &'a str,
    pub name: &'a str,
    pub body: &'a str,
}

pub fn parse_frame(text: &str) -> Option<Frame<'_>> {
    let (head, body) = text.split_once('?').unwrap_or((text, ""));
    let mut parts = head.splitn(3, ':');
    let kind = parts.next()?;
    let id = parts.next()?;
    let name = parts.next()?;
    if kind.is_empty() || id.is_empty() || name.is_empty() {
        return None;
    }
    Some(Frame { kind, id, name, body })
}

fn ack(id: &str, name: &str, body: &str) -> String {
    format!("ack:{id}:{name}?{body}")
}

/// Download one offered zip. `names` comes from the phone's send request.
pub async fn pull_zip(ip: IpAddr, port: u16, fingerprint_hex: &str, token: &str, dest: &Path) -> Result<Vec<String>> {
    let fingerprint = fingerprint_bytes(fingerprint_hex).context("file transfer fingerprint is not a certificate hash")?;
    let config = Arc::new(pinned_config(fingerprint));
    let connector = TlsConnector::from(config);
    let name = ServerName::try_from("localhost").context("server name")?;
    let tcp = tokio::time::timeout(Duration::from_secs(8), TcpStream::connect((ip, port)))
        .await
        .context("timed out reaching the phone")??;
    let tls = tokio::time::timeout(Duration::from_secs(8), connector.connect(name.clone(), tcp))
        .await
        .context("timed out during the file certificate check")??;
    let url = format!("wss://{ip}:{port}/websocket");
    let (mut socket, _) = tokio::time::timeout(Duration::from_secs(8), client_async(url, tls))
        .await
        .context("timed out opening the file channel")??;

    let opened = read_text(&mut socket).await.context("the phone closed the file channel")?;
    let version = parse_frame(&opened).context("the phone sent an unexpected file greeting")?;
    if version.name != "versionNegotiation" {
        anyhow::bail!("the phone sent {} instead of a version", version.name);
    }
    socket
        .send(Message::Text(ack(version.id, version.name, r#"{"version":1}"#).into()))
        .await?;

    let request = read_text(&mut socket).await.context("the phone closed before naming the files")?;
    let send = parse_frame(&request).context("the phone's file list was unreadable")?;
    if send.name != "sendRequest" {
        anyhow::bail!("the phone sent {} instead of a file list", send.name);
    }
    let names = file_names(send.body);
    let task_id = task_id(send.body).context("the phone's file list had no id")?;
    socket.send(Message::Text(ack(send.id, send.name, "{}").into())).await?;

    let tcp = tokio::time::timeout(Duration::from_secs(8), TcpStream::connect((ip, port)))
        .await
        .context("timed out downloading the files")??;
    let mut tls = tokio::time::timeout(Duration::from_secs(8), connector.connect(name, tcp))
        .await
        .context("timed out downloading the files")??;
    let path = format!(
        "/download?taskId={}&token={}",
        urlencoding_min(&task_id),
        urlencoding_min(token)
    );
    let host = ip.to_string();
    let request = format!("GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nAccept: */*\r\n\r\n");
    tls.write_all(request.as_bytes()).await?;
    let mut raw = Vec::new();
    let mut chunk = [0u8; 64 * 1024];
    loop {
        let read = tls.read(&mut chunk).await?;
        if read == 0 {
            break;
        }
        if raw.len() + read > MAX_ZIP + 64 * 1024 {
            anyhow::bail!("the phone offered a file larger than 512 MB");
        }
        raw.extend_from_slice(&chunk[..read]);
    }
    let (status, body) = split_http(&raw).context("the download was not an HTTP response")?;
    if status != 200 || body.len() < 4 || &body[..2] != b"PK" {
        anyhow::bail!("the phone did not return a zip (HTTP {status})");
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(dest, &body).with_context(|| format!("saving {}", dest.display()))?;

    let status_frame = format!(
        "action:Client1001:status?{{\"taskId\":{},\"type\":1,\"reason\":\"\"}}",
        serde_json::to_string(&task_id).unwrap_or_else(|_| "\"\"".into())
    );
    let _ = socket.send(Message::Text(status_frame.into())).await;
    let _ = read_text(&mut socket).await;
    Ok(names)
}

async fn read_text<S>(socket: &mut tokio_tungstenite::WebSocketStream<S>) -> Result<String>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let message = tokio::time::timeout(Duration::from_secs(20), socket.next())
        .await
        .context("timed out waiting for the phone")?
        .context("the phone closed the file channel")??;
    match message {
        Message::Text(text) => Ok(text.to_string()),
        Message::Close(_) => anyhow::bail!("the phone closed the file channel"),
        _ => anyhow::bail!("the phone sent a non-text file frame"),
    }
}

fn file_names(body: &str) -> Vec<String> {
    let value: serde_json::Value = serde_json::from_str(body).unwrap_or(serde_json::Value::Null);
    let mut names = Vec::new();
    if let Some(list) = value.get("file").and_then(|item| item.as_array()) {
        for item in list {
            if let Some(name) = item.get("name").and_then(|name| name.as_str()) {
                if !name.is_empty() {
                    names.push(name.to_string());
                }
            }
        }
    }
    if names.is_empty() {
        if let Some(name) = value.get("fileName").and_then(|name| name.as_str()) {
            if !name.is_empty() {
                names.push(name.to_string());
            }
        }
    }
    names
}

fn task_id(body: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    match &value["id"] {
        serde_json::Value::String(text) if !text.is_empty() => Some(text.clone()),
        serde_json::Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

fn urlencoding_min(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn split_http(raw: &[u8]) -> Option<(u16, Vec<u8>)> {
    let split = raw.windows(4).position(|window| window == b"\r\n\r\n")?;
    let header = std::str::from_utf8(&raw[..split]).ok()?;
    let status = header.lines().next()?.split_whitespace().nth(1)?.parse().ok()?;
    Some((status, raw[split + 4..].to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_phone_frame() {
        let frame = parse_frame(r#"action:Server1001:versionNegotiation?{"version":1}"#).unwrap();
        assert_eq!(frame.kind, "action");
        assert_eq!(frame.id, "Server1001");
        assert_eq!(frame.name, "versionNegotiation");
        assert_eq!(frame.body, r#"{"version":1}"#);
        assert_eq!(ack(frame.id, frame.name, r#"{"version":1}"#), r#"ack:Server1001:versionNegotiation?{"version":1}"#);
    }

    #[test]
    fn reads_the_file_list() {
        let body = r#"{"id":"task-9","fileName":"a.jpg","file":[{"name":"a.jpg"},{"name":"b.png"}]}"#;
        assert_eq!(task_id(body).as_deref(), Some("task-9"));
        assert_eq!(file_names(body), vec!["a.jpg".to_string(), "b.png".to_string()]);
    }

    #[test]
    fn splits_an_http_response() {
        let raw = b"HTTP/1.1 200 OK\r\nContent-Type: application/zip\r\n\r\nPK\x03\x04rest";
        let (status, body) = split_http(raw).unwrap();
        assert_eq!(status, 200);
        assert_eq!(&body[..2], b"PK");
    }
}
