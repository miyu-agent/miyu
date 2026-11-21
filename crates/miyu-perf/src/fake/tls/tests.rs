//! HTTPS 的两个口（施工 LW-1）：证书写成 PEM 读得回来；照核心那样只信任这一张、ALPN 先说 `h2` 的客户端，在两个口上各落到
//! 一种，回的和明文的口一样，收到的请求记着是从哪个口来的。

use std::sync::Arc;

use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, ServerName};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;
use tokio_rustls::client::TlsStream;

use super::{CERT, base64, pem};
use crate::fake::{Fake, Transport};

#[test]
fn base64_follows_the_standard() {
    for (bytes, text) in [
        ("", ""),
        ("f", "Zg=="),
        ("fo", "Zm8="),
        ("foo", "Zm9v"),
        ("foob", "Zm9vYg=="),
        ("foobar", "Zm9vYmFy"),
    ] {
        assert_eq!(base64(bytes.as_bytes()), text, "{bytes}");
    }
    assert_eq!(base64(&[0xff, 0xfe, 0x00]), "//4A");
}

#[test]
fn the_pem_reads_back_to_the_certificate() {
    let pem = pem();
    assert!(pem.starts_with("-----BEGIN CERTIFICATE-----\n"), "{pem}");
    assert!(pem.lines().all(|line| line.len() <= 64), "{pem}");
    let parsed = CertificateDer::from_pem_slice(pem.as_bytes()).unwrap();
    assert_eq!(parsed.as_ref(), CERT);
}

/// 照核心那样的客户端：只信任这一张，ALPN 先 `h2` 后 `http/1.1`；连上 `url` 的口、握完手。
async fn connect(url: &str) -> TlsStream<TcpStream> {
    let mut roots = rustls::RootCertStore::empty();
    roots.add(CertificateDer::from(CERT.to_vec())).unwrap();
    let mut config = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_root_certificates(roots)
    .with_no_client_auth();
    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    let address = url.trim_start_matches("https://").trim_end_matches("/v1");
    let socket = TcpStream::connect(address).await.unwrap();
    let name = ServerName::try_from("127.0.0.1").unwrap();
    TlsConnector::from(Arc::new(config))
        .connect(name, socket)
        .await
        .unwrap()
}

#[tokio::test]
async fn both_secure_ports_answer_and_say_where_the_request_came_from() {
    let mut fake = Fake::start(100).await.unwrap();
    let secure = fake.secure().await.unwrap();

    let mut tls = connect(&secure.http1).await;
    assert_eq!(tls.get_ref().1.alpn_protocol(), Some(&b"http/1.1"[..]));
    let body = "{\"text\":\"perf-tls-1\"}";
    let request = format!(
        "POST /v1/chat/completions HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    tls.write_all(request.as_bytes()).await.unwrap();
    let mut reply = Vec::new();
    // 回完对方关上；TLS 没发 close_notify 就断的算读完。
    drop(tls.read_to_end(&mut reply).await);
    let reply = String::from_utf8(reply).unwrap();
    assert!(reply.starts_with("HTTP/1.1 200 OK\r\n"), "{reply}");
    assert!(reply.ends_with("data: [DONE]\n\n"), "{reply}");
    let arrival = fake.request_with("perf-tls-1").await.unwrap();
    assert_eq!(arrival.via, Transport::Http1);
    assert_eq!(arrival.body, body.as_bytes());

    let tls = connect(&secure.h2).await;
    assert_eq!(tls.get_ref().1.alpn_protocol(), Some(&b"h2"[..]));
    let (client, connection) = h2::client::handshake(tls).await.unwrap();
    tokio::spawn(connection);
    let mut client = client.ready().await.unwrap();
    // 请求体比一个窗口（64 KiB）大：读的一方要还窗口才读得完。
    let body = format!("{{\"text\":\"{}perf-h2-1\"}}", "x".repeat(200_000));
    let request = http::Request::post("https://127.0.0.1/v1/chat/completions")
        .body(())
        .unwrap();
    let (response, mut sending) = client.send_request(request, false).unwrap();
    sending.reserve_capacity(body.len());
    sending
        .send_data(bytes::Bytes::from(body.clone()), true)
        .unwrap();
    let response = response.await.unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.headers()["content-type"],
        "text/event-stream",
        "{response:?}"
    );
    let mut incoming = response.into_body();
    let mut reply = Vec::new();
    while let Some(chunk) = incoming.data().await {
        let chunk = chunk.unwrap();
        incoming
            .flow_control()
            .release_capacity(chunk.len())
            .unwrap();
        reply.extend_from_slice(&chunk);
    }
    assert!(reply.ends_with(b"data: [DONE]\n\n"));
    let arrival = fake.request_with("perf-h2-1").await.unwrap();
    assert_eq!(arrival.via, Transport::H2);
    assert_eq!(arrival.body, body.as_bytes());
    assert!(arrival.answered.await.unwrap().at >= arrival.at);
}
