//! 假的软件源（施工 G-3）：测试自己的钥匙签索引，假服务器先后回索引和签名，核心照写好的系统配置找软件源、认这把钥匙。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_http::testkit::{Piece, Reply};
use miyu_session::testkit::Script;
use miyu_source::sign::{self, PublicKey};
use miyu_tool::Catalog;

use super::{Home, TOKEN, alice};

/// 新建一对钥匙：私钥，和只认它的公钥列表。
pub fn keys() -> (String, Vec<PublicKey>) {
    let made = sign::generate().expect("生成得了");
    let public = PublicKey::from_base64(&made.public).expect("读得成");
    (made.secret, vec![public])
}

/// 一份签好的索引：生成的时刻、几个包（编号、名字、说明），用 `secret` 签。交回索引和签名两份字节。
pub fn signed_entries(generated: &str, packages: &[Value], secret: &str) -> (Vec<u8>, Vec<u8>) {
    let index =
        serde_json::to_vec(&json!({"version": 1, "generated": generated, "packages": packages}))
            .expect("写得成");
    let signature =
        serde_json::to_vec(&sign::sign(&index, secret).expect("签得了")).expect("写得成");
    (index, signature)
}

/// 同 [`signed_entries`]，包只写编号。
pub fn signed(generated: &str, ids: &[&str], secret: &str) -> (Vec<u8>, Vec<u8>) {
    let packages: Vec<Value> = ids
        .iter()
        .map(|id| entry(id, json!({"en": id}), json!({})))
        .collect();
    signed_entries(generated, &packages, secret)
}

/// 索引里的一条：编号、名字、说明。
pub fn entry(id: &str, name: Value, summary: Value) -> Value {
    json!({"id": id, "version": "1.0", "name": name, "summary": summary,
           "origin": format!("github.com/miyu-agent/{id}"), "official": true,
           "files": [{"url": "https://example.invalid/x.miyu", "sha256": "ab", "size": 1}]})
}

/// 两份回应：索引、签名。
pub fn serving(pair: &(Vec<u8>, Vec<u8>)) -> Vec<Reply> {
    vec![
        Reply::stream(vec![Piece::Bytes(pair.0.clone())]),
        Reply::stream(vec![Piece::Bytes(pair.1.clone())]),
    ]
}

/// 系统配置里写了软件源、镜像，认 `keys` 的核心。
pub fn core(home: &Home, source: &str, mirrors: &[&str], keys: Vec<PublicKey>) -> Arc<Core> {
    let mirrors: Vec<String> = mirrors.iter().map(|m| format!("\"{m}\"")).collect();
    home.write(
        "system/config.toml",
        &format!(
            "[packages]\nsource = \"{source}\"\nmirrors = [{}]\n",
            mirrors.join(", ")
        ),
    );
    let config = miyu_endpoint::config::Config::load(
        &home.root,
        &alice(),
        None,
        miyu_endpoint::settings::SourceSettings::ITEMS.to_vec(),
        miyu_endpoint::config::Environment::of(&[]),
    );
    Arc::new(
        home.core_full(&Script::new([]), Catalog::default(), None, TOKEN)
            .with_config(config)
            .with_source_keys(keys),
    )
}
