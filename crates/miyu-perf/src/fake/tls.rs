//! 假模型的 HTTPS（施工 LW-1）：TLS 用 rustls（ring），证书是仓库里那张给 `127.0.0.1` 的自签测试证书（`tls/` 下，和
//! `miyu-http` 的测试用的同一张，拷一份过来不跨 crate 引路径）。核心照 `SSL_CERT_FILE` 信任它：排函数那一场把这个变量
//! 指到「系统的证书库 + 这一张」（[`pem`]），webpki 认自签的叶子当信任锚（`CA:FALSE`，主机名对得上就验得过）。
//!
//! 两个口各一份配置，只差 ALPN：一个只给 `http/1.1`，一个只给 `h2`。核心的 ALPN 先说 `h2` 后说 `http/1.1`，各落到一种。

use std::sync::Arc;

use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

use super::shared::Shared;
use super::{Transport, http1, http2};

/// 自签的测试证书（`CN=miyu test, not a real certificate`，给 `127.0.0.1` 的，一百年）。
const CERT: &[u8] = include_bytes!("../../tls/self-signed.cert.der");

/// 它的私钥（PKCS#8，P-256）。只给测试用，公开在仓库里。
const KEY: &[u8] = include_bytes!("../../tls/self-signed.key.der");

/// `via` 那个口的 TLS 配置：`Http1` 的 ALPN 只给 `http/1.1`，`H2` 的只给 `h2`。
///
/// # Errors
///
/// 证书和私钥配不上；`via` 是明文的口。
pub(super) fn acceptor(via: Transport) -> Result<TlsAcceptor, String> {
    let alpn: &[u8] = match via {
        Transport::Http1 => b"http/1.1",
        Transport::H2 => b"h2",
        Transport::Plain => return Err("明文的口不配 TLS".to_string()),
    };
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(KEY.to_vec()));
    let mut config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(|e| format!("TLS 的协议版本配不上：{e}"))?
    .with_no_client_auth()
    .with_single_cert(vec![CertificateDer::from(CERT.to_vec())], key)
    .map_err(|e| format!("测试证书和私钥配不上：{e}"))?;
    config.alpn_protocols = vec![alpn.to_vec()];
    Ok(TlsAcceptor::from(Arc::new(config)))
}

/// 一直收 `listener` 上的连接：握完 TLS 交给 `via` 那种传输。握手没成的（核心不信任证书、ALPN 谈不拢）丢掉，核心那边
/// 照它的请求出错报出来。
pub(super) async fn accept(
    listener: TcpListener,
    acceptor: TlsAcceptor,
    shared: Arc<Shared>,
    via: Transport,
) {
    while let Ok((socket, _)) = listener.accept().await {
        let acceptor = acceptor.clone();
        let shared = Arc::clone(&shared);
        tokio::spawn(async move {
            let Ok(tls) = acceptor.accept(socket).await else {
                return;
            };
            match via {
                Transport::H2 => http2::serve(tls, shared).await,
                _ => http1::serve(tls, shared, via).await,
            }
        });
    }
}

/// 测试证书写成 PEM：核心的 `SSL_CERT_FILE` 要的写法（`rustls-native-certs` 读的就是 PEM）。
pub fn pem() -> String {
    let encoded = base64(CERT);
    let mut pem = String::from("-----BEGIN CERTIFICATE-----\n");
    for line in encoded.as_bytes().chunks(64) {
        pem.push_str(&String::from_utf8_lossy(line));
        pem.push('\n');
    }
    pem.push_str("-----END CERTIFICATE-----\n");
    pem
}

/// 标准的 base64（RFC 4648，带 `=` 补齐）。只编这一张证书用，不为它拉一个包。
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut text = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for group in bytes.chunks(3) {
        let n = group
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, byte)| n | (u32::from(*byte) << (16 - 8 * i)));
        for i in 0..4 {
            let sextet = if i <= group.len() {
                ALPHABET[((n >> (18 - 6 * i)) & 63) as usize]
            } else {
                b'='
            };
            text.push(char::from(sextet));
        }
    }
    text
}

#[cfg(test)]
mod tests;
