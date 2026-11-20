//! 对方的证书不被信任（施工 TLS-1，`internal/blueprint/http.md`「出错」）：当场报错、`other`、不重试，报错里说是证书不被信任。
//! 原来没有状态的失败一律落成可重试，内核照它重来五次，三十秒以后才报。
//!
//! 假服务器用一张自签的测试证书（`support/self-signed.cert.der`、`self-signed.key.der`，只给这份测试用）做 TLS 握手：
//! 客户端验不过证书，握手到这里就断。

use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use miyu_http::{Endpoint, Outcome, Progress};
use miyu_kernel::event::ErrorClass;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

use crate::send::{never, run};

/// 自签的测试证书（`CN=miyu test, not a real certificate`，给 `127.0.0.1` 的）和它的私钥。
const CERT: &[u8] = include_bytes!("support/self-signed.cert.der");
const KEY: &[u8] = include_bytes!("support/self-signed.key.der");

/// 在本机起一个只做 TLS 握手的服务器，交回端口和握了几次手。线程跟着测试进程结束。
fn tls_server() -> (u16, Arc<AtomicUsize>) {
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(KEY.to_vec()));
    let config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .expect("版本配得上")
    .with_no_client_auth()
    .with_single_cert(vec![CertificateDer::from(CERT.to_vec())], key)
    .expect("证书和私钥对得上");
    let config = Arc::new(config);
    let listener = TcpListener::bind("127.0.0.1:0").expect("占得到端口");
    let port = listener.local_addr().expect("有地址").port();
    let handshakes = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&handshakes);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            seen.fetch_add(1, Ordering::SeqCst);
            let Ok(mut tls) = rustls::ServerConnection::new(Arc::clone(&config)) else {
                continue;
            };
            // 握到客户端拒了证书、断开为止；握成了（不该）也只握手，不回话。
            while tls.is_handshaking() && tls.complete_io(&mut stream).is_ok() {}
        }
    });
    (port, handshakes)
}

#[tokio::test]
async fn an_untrusted_certificate_is_other_and_says_so() {
    let (port, handshakes) = tls_server();
    let endpoint = Endpoint::new(format!("https://127.0.0.1:{port}/v1"), "sk-test");
    let (progress, outcome) = run(&endpoint, Duration::from_secs(10), never()).await;
    // 照样先报发出去了。
    assert!(matches!(progress.first(), Some(Progress::Sent { .. })));
    let Outcome::Ended {
        error: Some(error), ..
    } = outcome
    else {
        panic!("应该出错：{outcome:?}");
    };
    // 重来也一样：`other`，不重试。
    assert_eq!(
        error.error.class,
        ErrorClass::Unclassified,
        "{}",
        error.error.message
    );
    assert_eq!(error.error.status, None);
    let message = &error.error.message;
    assert!(message.starts_with("证书不被信任："), "{message}");
    assert!(message.contains("invalid peer certificate"), "{message}");
    // 不带地址、不带 key。
    assert!(!message.contains("://"), "{message}");
    assert!(!message.contains("sk-test"), "{message}");
    assert_eq!(handshakes.load(Ordering::SeqCst), 1, "只连了一次");
}
