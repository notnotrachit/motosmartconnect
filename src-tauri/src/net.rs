//! TLS and local addresses.
//!
//! The phone pins our certificate by fingerprint, so the server certificate is the
//! one stored with the identity. When this host dials the phone's discovery socket
//! it does not pin the phone certificate: the phone is the one that checks `fp`.

use anyhow::{Context, Result};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, ServerConfig, SignatureScheme};
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use tokio::net::TcpListener;

pub fn install_crypto() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

pub fn server_config(cert_pem: &str, key_pem: &str) -> Result<Arc<ServerConfig>> {
    let mut cert_reader = std::io::Cursor::new(cert_pem.as_bytes());
    let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut cert_reader).collect::<Result<Vec<_>, _>>()?;
    let mut key_reader = std::io::Cursor::new(key_pem.as_bytes());
    let key: PrivateKeyDer<'static> = rustls_pemfile::private_key(&mut key_reader)?.context("identity has no private key")?;
    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .context("building the tls listener")?;
    Ok(Arc::new(config))
}

#[derive(Debug)]
struct AcceptAnyCert;

impl ServerCertVerifier for AcceptAnyCert {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        rustls::crypto::ring::default_provider().signature_verification_algorithms.supported_schemes()
    }
}

pub fn phone_client_config() -> ClientConfig {
    ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AcceptAnyCert))
        .with_no_client_auth()
}

pub async fn bind_listener() -> Result<(TcpListener, u16)> {
    let mut last_error = None;
    for port in 52020u16..=52040 {
        match TcpListener::bind((Ipv4Addr::UNSPECIFIED, port)).await {
            Ok(listener) => return Ok((listener, port)),
            Err(err) => last_error = Some(err),
        }
    }
    match last_error {
        Some(err) => Err(err).context("ports 52020-52040 are all in use"),
        None => anyhow::bail!("ports 52020-52040 are all in use"),
    }
}

pub fn local_ipv4s() -> Vec<String> {
    let mut found = Vec::new();
    let mut extra = Vec::new();
    if let Ok(interfaces) = local_ip_address::list_afinet_netifas() {
        for (name, addr) in interfaces {
            if let IpAddr::V4(ipv4) = addr {
                if usable(ipv4) {
                    let text = ipv4.to_string();
                    // Bridges and virtual adapters are not the network the phone is on.
                    if name.starts_with("en") {
                        if !found.contains(&text) {
                            found.push(text);
                        }
                    } else if !extra.contains(&text) {
                        extra.push(text);
                    }
                }
            }
        }
    }
    if found.is_empty() {
        found = extra;
    }
    if found.is_empty() {
        found.push("127.0.0.1".to_string());
    }
    found
}

fn usable(ip: Ipv4Addr) -> bool {
    !ip.is_loopback() && !ip.is_link_local() && !ip.is_unspecified() && !ip.is_multicast()
}
