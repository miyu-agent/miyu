//! 晚一点造的验证器（施工 V-33）：第一次要验对方的证书时才读证书、造 rustls 自己的 `WebPkiServerVerifier`，之后每一次
//! 都原样交给它。
//!
//! 只是推迟，不改怎么验：证书链、主机名、有效期、签名，全是那个验证器验的，证书库、算法和原来 `with_root_certificates`
//! 造的一样（`super::verifier`）。不用造验证器就答得出来的只有一样：支持哪些签名算法，那是密码学实现自带的表，验证器答的
//! 也是它（`WebPkiServerVerifier::supported_verify_schemes`）。要不要原始公钥、有没有根证书的提示，那个验证器照默认答，
//! 这里也照默认。
//!
//! 签名的两样也交给它：rustls 先验证书、再验签名，到这一步它已经造好了。

use std::fmt;
use std::sync::{Arc, OnceLock};

use rustls::client::WebPkiServerVerifier;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::WebPkiSupportedAlgorithms;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, Error, SignatureScheme};

/// 第一次用时才造的验证器。
pub(crate) struct Deferred {
    /// 支持哪些签名算法：和造出来的验证器用的同一张表。
    algorithms: WebPkiSupportedAlgorithms,
    /// 怎么造。
    make: fn() -> Arc<WebPkiServerVerifier>,
    /// 造好的。
    made: OnceLock<Arc<WebPkiServerVerifier>>,
}

impl Deferred {
    /// 照 `make` 造，造出来的验证器用的签名算法是 `algorithms`。
    pub(crate) fn new(
        algorithms: WebPkiSupportedAlgorithms,
        make: fn() -> Arc<WebPkiServerVerifier>,
    ) -> Deferred {
        Deferred {
            algorithms,
            make,
            made: OnceLock::new(),
        }
    }

    /// 造好的那个；没造的这时造。几个连接同时第一次验的，只造一次。
    fn verifier(&self) -> &WebPkiServerVerifier {
        self.made.get_or_init(self.make)
    }

    /// 造过没有。
    #[cfg(test)]
    pub(crate) fn made(&self) -> bool {
        self.made.get().is_some()
    }
}

impl fmt::Debug for Deferred {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Deferred")
            .field("made", &self.made.get().is_some())
            .finish_non_exhaustive()
    }
}

impl ServerCertVerifier for Deferred {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        self.verifier().verify_server_cert(
            end_entity,
            intermediates,
            server_name,
            ocsp_response,
            now,
        )
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        self.verifier().verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        self.verifier().verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.algorithms.supported_schemes()
    }
}

#[cfg(test)]
mod tests;
