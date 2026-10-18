//! 索引的签名（施工 G-2 上，设计 `37-软件源.md` 第三节第 2 条）：Ed25519 签 `index.json` 的字节，`index.json.sig` 是
//! `{"key": "<公钥编号>", "signature": "<base64>"}`。验不过、钥匙不认识的整份不收。
//!
//! - 用 ring（rustls 已经带进来的那一份），不另加密码库。
//! - 公钥编号是公钥 SHA-256 的前 16 位十六进制：换钥匙时新旧都列着，照编号挑。
//! - 私钥是 PKCS#8 文档的 base64，只在收录的 Actions 密钥和项目主人手里；公钥编进 `miyu`（[`OFFICIAL`]）。

use std::fmt;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use ring::rand::SystemRandom;
use ring::signature::{ED25519, Ed25519KeyPair, KeyPair, UnparsedPublicKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// 官方软件源的公钥：编号、base64。项目主人建好钥匙以后填进来（施工 G-7）。
pub const OFFICIAL: &[(&str, &str)] = &[];

/// 一把公钥。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicKey {
    /// 编号：[`key_id`]。
    pub id: String,
    /// 32 字节的 Ed25519 公钥。
    pub bytes: Vec<u8>,
}

impl PublicKey {
    /// 照 base64 读；读不成、不是 32 字节的没有。编号照公钥算。
    pub fn from_base64(text: &str) -> Option<PublicKey> {
        let bytes = STANDARD.decode(text.trim()).ok()?;
        (bytes.len() == 32).then(|| PublicKey {
            id: key_id(&bytes),
            bytes,
        })
    }
}

/// 编进来的官方公钥；写得不对的跳过（门禁的测试查它们都读得成、编号对得上）。
pub fn official() -> Vec<PublicKey> {
    OFFICIAL
        .iter()
        .filter_map(|(_, text)| PublicKey::from_base64(text))
        .collect()
}

/// `index.json.sig` 的写法。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signature {
    /// 用哪把钥匙签的：公钥编号。
    pub key: String,
    /// 签名，base64。
    pub signature: String,
}

/// 新建的一对钥匙。
#[derive(Debug, Clone)]
pub struct Generated {
    /// 私钥：PKCS#8 文档的 base64。只给项目主人，不进仓库。
    pub secret: String,
    /// 公钥的 base64：编进 [`OFFICIAL`]。
    pub public: String,
    /// 公钥编号。
    pub id: String,
}

/// 签、验不成的原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// 拿不到随机数，生成不了钥匙。
    Random,
    /// 私钥读不成。
    BadSecret,
    /// 签名文件不是这个写法。
    BadSignatureFile(String),
    /// 签名用的钥匙不在认的那几把里。
    UnknownKey(String),
    /// 签名对不上内容。
    Mismatch,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Random => write!(f, "no randomness for a new key"),
            Error::BadSecret => write!(f, "unreadable secret key"),
            Error::BadSignatureFile(why) => write!(f, "malformed signature file: {why}"),
            Error::UnknownKey(id) => write!(f, "signed with an unknown key {id}"),
            Error::Mismatch => write!(f, "the signature does not match the index"),
        }
    }
}

impl std::error::Error for Error {}

/// 公钥编号：公钥 SHA-256 的前 16 位十六进制。
pub fn key_id(public: &[u8]) -> String {
    Sha256::digest(public)
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// 新建一对钥匙。
///
/// # Errors
///
/// 拿不到随机数。
pub fn generate() -> Result<Generated, Error> {
    let document =
        Ed25519KeyPair::generate_pkcs8(&SystemRandom::new()).map_err(|_| Error::Random)?;
    let pair = Ed25519KeyPair::from_pkcs8(document.as_ref()).map_err(|_| Error::Random)?;
    let public = pair.public_key().as_ref();
    Ok(Generated {
        secret: STANDARD.encode(document.as_ref()),
        public: STANDARD.encode(public),
        id: key_id(public),
    })
}

/// 用私钥 `secret`（PKCS#8 的 base64）签 `bytes`。
///
/// # Errors
///
/// 私钥读不成。
pub fn sign(bytes: &[u8], secret: &str) -> Result<Signature, Error> {
    let document = STANDARD
        .decode(secret.trim())
        .map_err(|_| Error::BadSecret)?;
    let pair = Ed25519KeyPair::from_pkcs8(&document).map_err(|_| Error::BadSecret)?;
    Ok(Signature {
        key: key_id(pair.public_key().as_ref()),
        signature: STANDARD.encode(pair.sign(bytes).as_ref()),
    })
}

/// 照 `keys` 验 `bytes` 的签名文件 `signature`（`index.json.sig` 的字节），交回用的哪把钥匙的编号。
///
/// # Errors
///
/// 签名文件写法不对；钥匙不认识；签名对不上。
pub fn verify(bytes: &[u8], signature: &[u8], keys: &[PublicKey]) -> Result<String, Error> {
    let signature: Signature = serde_json::from_slice(signature)
        .map_err(|error| Error::BadSignatureFile(error.to_string()))?;
    let key = keys
        .iter()
        .find(|key| key.id == signature.key)
        .ok_or_else(|| Error::UnknownKey(signature.key.clone()))?;
    let raw = STANDARD
        .decode(signature.signature.trim())
        .map_err(|error| Error::BadSignatureFile(error.to_string()))?;
    UnparsedPublicKey::new(&ED25519, &key.bytes)
        .verify(bytes, &raw)
        .map_err(|_| Error::Mismatch)?;
    Ok(key.id.clone())
}

#[cfg(test)]
mod tests;
