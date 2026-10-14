//! 整个进程共用的一份 TLS 配置（施工 V-5 上，`docs/blueprint/http.md`「客户端」）。
//!
//! reqwest 每造一个客户端，都把系统的证书从头读一遍：Linux 上要打开三百多个文件。核心起来时造四个客户端，就是十几毫秒，
//! 内存里还存了四份。链接卡片每钉一个地址造一个客户端，每次也都要重读。
//!
//! 现在第一次要时读一次，拼成一份配置，各个客户端都用它的拷贝。拷贝里的证书库和会话缓存是同一份。拼法照 reqwest 自己
//! 的：先放 webpki 自带的根证书，再放系统的；读不懂的那一张跳过；协议版本全开；ALPN 先 `h2` 后 `http/1.1`。

use std::sync::{Arc, OnceLock};

#[cfg(test)]
pub(crate) static BUILT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// 共用的那一份的拷贝：第一次要时读证书、拼好，之后直接拷。
///
/// # Panics
///
/// 实际不会：ring 的实现支持 rustls 的全部协议版本。
pub fn tls() -> rustls::ClientConfig {
    static SHARED: OnceLock<rustls::ClientConfig> = OnceLock::new();
    SHARED.get_or_init(build).clone()
}

/// 读证书、拼配置。照 reqwest 0.12 开着 `rustls-tls-native-roots`、`rustls-tls-webpki-roots`、`http2` 时自己拼的那样。
fn build() -> rustls::ClientConfig {
    #[cfg(test)]
    BUILT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    for cert in rustls_native_certs::load_native_certs().certs {
        // 读不懂的那一张跳过，和 reqwest 一样：一张坏证书不该让整个客户端造不出来。
        drop(roots.add(cert));
    }
    let provider = rustls::crypto::CryptoProvider::get_default()
        .cloned()
        .unwrap_or_else(|| Arc::new(rustls::crypto::ring::default_provider()));
    let mut config = rustls::ClientConfig::builder_with_provider(provider)
        .with_protocol_versions(rustls::ALL_VERSIONS)
        .expect("ring 的实现支持 rustls 的全部协议版本")
        .with_root_certificates(roots)
        .with_no_client_auth();
    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    config
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use super::{BUILT, tls};

    #[test]
    fn the_certificates_are_read_once_and_every_copy_speaks_h2_first() {
        let first = tls();
        let second = tls();
        drop(crate::client(crate::Proxy::Off).expect("造得出客户端"));
        drop(crate::fetcher(crate::Proxy::FromEnvironment).expect("造得出客户端"));
        assert_eq!(BUILT.load(Ordering::Relaxed), 1, "证书只读一次");
        for config in [&first, &second] {
            assert_eq!(
                config.alpn_protocols,
                vec![b"h2".to_vec(), b"http/1.1".to_vec()]
            );
        }
    }
}
