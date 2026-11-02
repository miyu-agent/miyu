//! 整个进程共用的一份 TLS 配置（施工 V-5 上，`docs/blueprint/http.md`「客户端」）。
//!
//! reqwest 每造一个客户端，都把系统的证书从头读一遍：Linux 上要打开三百多个文件。核心起来时造四个客户端，就是十几毫秒，
//! 内存里还存了四份。链接卡片每钉一个地址造一个客户端，每次也都要重读。
//!
//! 现在拼一份配置，各个客户端都用它的拷贝。拷贝里的证书库和会话缓存是同一份。拼法照 reqwest 自己的：先放 webpki 自带的
//! 根证书，再放系统的；读不懂的那一张跳过；协议版本全开；ALPN 先 `h2` 后 `http/1.1`。
//!
//! 证书等第一次真要验对方的证书时才读（施工 V-33，[`Deferred`]）：核心起来时造好几个客户端（模型请求、拉列表、汇率、
//! 更新目录），可一回一次网都不上的核心，原来也要读三百多个文件、常驻约 100 KB 的证书库，占冷启动 CPU 的三四成之一。
//! 验证照旧全交给 rustls 自己的 `WebPkiServerVerifier`，证书库、算法、结果一样，只是晚一点造。

use std::sync::{Arc, OnceLock};

use rustls::client::WebPkiServerVerifier;
use rustls::crypto::CryptoProvider;

mod deferred;

use deferred::Deferred;

#[cfg(test)]
pub(crate) static BUILT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// 共用的那一份的拷贝：第一次要时拼好，之后直接拷。证书不在这时读，第一次验对方的证书时才读（施工 V-33）。
///
/// # Panics
///
/// 实际不会：ring 的实现支持 rustls 的全部协议版本。
pub fn tls() -> rustls::ClientConfig {
    static SHARED: OnceLock<rustls::ClientConfig> = OnceLock::new();
    SHARED.get_or_init(build).clone()
}

/// 拼配置。照 reqwest 0.12 开着 `rustls-tls-native-roots`、`rustls-tls-webpki-roots`、`http2` 时自己拼的那样，只是验证器
/// 换成晚一点造的同一种（`with_root_certificates` 造的也是 `WebPkiServerVerifier::new_without_revocation`）。
fn build() -> rustls::ClientConfig {
    let provider = provider();
    let verifier = Arc::new(Deferred::new(
        provider.signature_verification_algorithms,
        verifier,
    ));
    let mut config = rustls::ClientConfig::builder_with_provider(provider)
        .with_protocol_versions(rustls::ALL_VERSIONS)
        .expect("ring 的实现支持 rustls 的全部协议版本")
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();
    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    config
}

/// 用哪一套密码学实现：进程装了默认的就用它，没有的用 ring。
fn provider() -> Arc<CryptoProvider> {
    CryptoProvider::get_default()
        .cloned()
        .unwrap_or_else(|| Arc::new(rustls::crypto::ring::default_provider()))
}

/// 读证书、造验证器：webpki 自带的根证书，再加系统的，读不懂的那一张跳过。[`Deferred`] 第一次验证时调一次。参数和
/// `with_root_certificates` 造的那个一样（`new_without_revocation`）：不给吊销列表，查不到吊销状态的放行。
///
/// # Panics
///
/// 实际不会：webpki 自带的根证书不是空的。
fn verifier() -> Arc<WebPkiServerVerifier> {
    #[cfg(test)]
    BUILT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    WebPkiServerVerifier::builder_with_provider(Arc::new(roots()), provider())
        .allow_unknown_revocation_status()
        .build()
        .expect("webpki 自带的根证书不是空的")
}

/// webpki 自带的根证书，再加系统的；读不懂的那一张跳过，和 reqwest 一样：一张坏证书不该让整个客户端造不出来。
fn roots() -> rustls::RootCertStore {
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    for cert in rustls_native_certs::load_native_certs().certs {
        drop(roots.add(cert));
    }
    roots
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use super::{BUILT, tls};

    /// 拼配置、造客户端都不读证书（施工 V-33：第一次验对方的证书时才读，`deferred/tests.rs` 守着）；每一份都先说 `h2`。
    #[test]
    fn making_clients_reads_no_certificates_and_every_copy_speaks_h2_first() {
        let first = tls();
        let second = tls();
        drop(crate::client(crate::Proxy::Off).expect("造得出客户端"));
        drop(crate::fetcher(crate::Proxy::FromEnvironment).expect("造得出客户端"));
        assert_eq!(
            BUILT.load(Ordering::Relaxed),
            0,
            "还没验过谁的证书，一张都不读"
        );
        for config in [&first, &second] {
            assert_eq!(
                config.alpn_protocols,
                vec![b"h2".to_vec(), b"http/1.1".to_vec()]
            );
        }
    }

    /// 真上网验一遍（不进门禁，本机手动跑：`cargo test -p miyu-http -- --ignored tls`）：正常的站连得上，过期的、主机名
    /// 不对的、自签的、来历不明的根签的都被拒。
    #[tokio::test]
    #[ignore = "要连外网"]
    async fn real_sites_pass_and_bad_certificates_are_refused() {
        let client = crate::client(crate::Proxy::FromEnvironment).expect("造得出客户端");
        let ok = client.get("https://example.com/").send().await;
        assert!(ok.is_ok(), "正常的站：{ok:?}");
        for bad in [
            "https://expired.badssl.com/",
            "https://wrong.host.badssl.com/",
            "https://self-signed.badssl.com/",
            "https://untrusted-root.badssl.com/",
        ] {
            let got = client.get(bad).send().await;
            assert!(got.is_err(), "{bad} 该被拒：{got:?}");
        }
    }
}
