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
    core_holding(home, source, mirrors, keys, &[])
}

/// 同 [`core`]，`packages.hold` 写这几个（施工 G-5）。
pub fn core_holding(
    home: &Home,
    source: &str,
    mirrors: &[&str],
    keys: Vec<PublicKey>,
    hold: &[&str],
) -> Arc<Core> {
    let quoted = |items: &[&str]| -> String {
        items
            .iter()
            .map(|item| format!("\"{item}\""))
            .collect::<Vec<_>>()
            .join(", ")
    };
    home.write(
        "system/config.toml",
        &format!(
            "[packages]\nsource = \"{source}\"\nmirrors = [{}]\nhold = [{}]\n",
            quoted(mirrors),
            quoted(hold)
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

/// 打一个真的包：编号、版本、依赖，交回文件名、字节（包文件夹在工作目录的 `src/<编号>-<版本>/<编号>`）。
pub fn packed(home: &Home, id: &str, version: &str, depends: &[&str]) -> (String, Vec<u8>) {
    let folder = home
        .work
        .join("src")
        .join(format!("{id}-{version}"))
        .join(id);
    std::fs::create_dir_all(&folder).expect("建得了");
    let mut text = format!("[package]\nversion = \"{version}\"\nname = {{ en = \"{id}\" }}\n");
    if !depends.is_empty() {
        let depends: Vec<String> = depends.iter().map(|d| format!("\"{d}\"")).collect();
        text.push_str(&format!(
            "\n[depends]\npackages = [{}]\n",
            depends.join(", ")
        ));
    }
    std::fs::write(folder.join("package.toml"), text).expect("写得进");
    let name = miyu_source::archive::file_name(id, version, None);
    let out = home.work.join("out").join(&name);
    std::fs::create_dir_all(out.parent().expect("有上一层")).expect("建得了");
    miyu_source::archive::pack(&folder, id, &out).expect("打得成");
    (name, std::fs::read(&out).expect("在"))
}

/// 索引里的一条：照真的包文件写大小和哈希；原址是到不了的，下载走镜像的 `files/`。
pub fn file_entry(id: &str, version: &str, depends: &[&str], file: &(String, Vec<u8>)) -> Value {
    json!({"id": id, "version": version, "name": {"en": id}, "origin": format!("github.com/miyu-agent/{id}"),
           "official": true, "depends": depends,
           "files": [{"url": format!("https://example.invalid/{}", file.0), "size": file.1.len(),
                      "sha256": sha256(&file.1)}]})
}

/// SHA-256，小写十六进制。
pub fn sha256(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// 一份回应：这些字节。
pub fn bytes(body: &[u8]) -> Reply {
    Reply::stream(vec![Piece::Bytes(body.to_vec())])
}

/// 发一条请求，收下回应之前推来的所有通知。
pub async fn call_collecting(
    client: &mut super::Client,
    method: &str,
    params: Value,
) -> (Value, Vec<Value>) {
    let request = json!({"jsonrpc": "2.0", "id": "c", "method": method, "params": params});
    client.line(&request.to_string()).await;
    let mut pushed = Vec::new();
    loop {
        let next = client.next().await.expect("有回应");
        if next.get("id").is_some() {
            return (next, pushed);
        }
        pushed.push(next);
    }
}
