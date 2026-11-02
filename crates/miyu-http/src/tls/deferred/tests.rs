//! 晚一点造的验证器（施工 V-33）：第一次验证书时才造、只造一次；每一样答的都和直接造的那个一模一样。验签名的两样是
//! 原样转过去的一行，这里不比：rustls 的 `DigitallySignedStruct` 在 crate 外面造不出来。

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use rustls::client::WebPkiServerVerifier;
use rustls::client::danger::ServerCertVerifier;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};

use super::Deferred;
use crate::tls::{provider, roots};

static MADE: AtomicUsize = AtomicUsize::new(0);

/// 和 `super::super::verifier` 同一种造法（不碰全进程的计数），每造一次记一次。
fn counted() -> Arc<WebPkiServerVerifier> {
    MADE.fetch_add(1, Ordering::SeqCst);
    direct()
}

fn direct() -> Arc<WebPkiServerVerifier> {
    WebPkiServerVerifier::builder_with_provider(Arc::new(roots()), provider())
        .allow_unknown_revocation_status()
        .build()
        .expect("webpki 自带的根证书不是空的")
}

fn deferred(make: fn() -> Arc<WebPkiServerVerifier>) -> Deferred {
    Deferred::new(provider().signature_verification_algorithms, make)
}

/// 固定的时刻：两边验同一张证书，结果不跟着跑测试的时刻变。
fn at() -> UnixTime {
    UnixTime::since_unix_epoch(Duration::from_secs(1_790_000_000))
}

/// 要验的几样：读不懂的、空的，和系统、webpki 里真实的几张根证书当成对方的证书（不是给这个主机名的、是 CA）。
fn certificates() -> Vec<CertificateDer<'static>> {
    let mut found = vec![
        CertificateDer::from(b"not a certificate".to_vec()),
        CertificateDer::from(Vec::new()),
    ];
    found.extend(
        rustls_native_certs::load_native_certs()
            .certs
            .into_iter()
            .take(5),
    );
    found
}

#[test]
fn it_is_made_at_the_first_verification_and_only_once() {
    let verifier = deferred(counted);
    assert!(!verifier.made(), "造出来时不造");
    let schemes = verifier.supported_verify_schemes();
    assert!(!schemes.is_empty());
    assert!(!verifier.made(), "问支持哪些算法不用造");
    let name = ServerName::try_from("example.com").expect("合法的主机名");
    let before = MADE.load(Ordering::SeqCst);
    for certificate in certificates() {
        drop(verifier.verify_server_cert(&certificate, &[], &name, &[], at()));
    }
    assert!(verifier.made(), "验过证书就造好了");
    assert_eq!(MADE.load(Ordering::SeqCst) - before, 1, "只造一次");
}

#[test]
fn every_answer_is_the_same_as_the_verifier_made_up_front() {
    let late = deferred(direct);
    let now = direct();
    assert_eq!(
        late.supported_verify_schemes(),
        now.supported_verify_schemes()
    );
    assert_eq!(
        late.requires_raw_public_keys(),
        now.requires_raw_public_keys()
    );
    assert_eq!(
        late.root_hint_subjects().is_some(),
        now.root_hint_subjects().is_some()
    );
    let names = ["example.com", "localhost", "127.0.0.1"];
    for certificate in certificates() {
        for name in names {
            let name = ServerName::try_from(name).expect("合法的主机名");
            let a = late.verify_server_cert(&certificate, &[], &name, &[], at());
            let b = now.verify_server_cert(&certificate, &[], &name, &[], at());
            assert_eq!(format!("{a:?}"), format!("{b:?}"), "{name:?}");
            let chain = [certificate.clone()];
            let a = late.verify_server_cert(&certificate, &chain, &name, b"ocsp", at());
            let b = now.verify_server_cert(&certificate, &chain, &name, b"ocsp", at());
            assert_eq!(format!("{a:?}"), format!("{b:?}"), "带中间证书 {name:?}");
        }
    }
}
