//! 核心起来时推旧包是怎么来的（施工 PK-6，[`infer`]）：配置里加的源算社区；官方源装的照本机官方索引里编号、来源仓库都对得上的那一条
//! 认官方、社区；本机文件照记的路径认；推不出来的不写；已经记了的不动。索引验签不过的当没有索引，不看索引的照推。

use miyu_source::sign::{self, PublicKey};
use miyu_store::packages::local::{self, Desc, Kind};
use miyu_store::resources::ResourceRoot;
use serde_json::json;

use super::*;
use crate::test_support::{resources, temp_root};

/// 一份 PK-6 以前的 `desc`：编号、来源仓库、软件源的名字、从哪个路径装的，没有 `kind`。
fn old(id: &str, origin: Option<&str>, feed: Option<&str>, source: &str) -> Desc {
    Desc {
        id: id.to_string(),
        version: None,
        installed: "2026-10-01T00:00:00.000Z".to_string(),
        source: source.to_string(),
        size: 0,
        origin: origin.map(str::to_string),
        feed: feed.map(str::to_string),
        kind: None,
    }
}

/// 官方索引的一条：编号、来源仓库、是不是官方收录的。
fn entry(id: &str, origin: &str, official: bool) -> serde_json::Value {
    json!({"id": id, "version": "1.0", "name": {"en": id}, "summary": {}, "origin": origin, "official": official,
           "files": [{"url": "https://example.invalid/x.miyu", "sha256": "ab", "size": 1}]})
}

/// 签好官方索引，放进 `dir`；交回认它的公钥。
fn index(dir: &Path, entries: &[serde_json::Value]) -> Vec<PublicKey> {
    let made = sign::generate().expect("生成得了");
    let bytes = serde_json::to_vec(
        &json!({"version": 1, "generated": "2026-11-01T00:00:00Z", "packages": entries}),
    )
    .expect("写得成");
    let signature =
        serde_json::to_vec(&sign::sign(&bytes, &made.secret).expect("签得了")).expect("写得成");
    std::fs::create_dir_all(dir).expect("建得了");
    std::fs::write(dir.join("index.json"), bytes).expect("写得进");
    std::fs::write(dir.join("index.json.sig"), signature).expect("写得进");
    vec![PublicKey::from_base64(&made.public).expect("读得成")]
}

/// 本地库里的老包、官方索引，推一遍，交回每个编号记下的 `kind`。
fn inferred(name: &str, trusted: bool) -> Vec<(String, Option<Kind>)> {
    let (root, _dir) = temp_root(name);
    let places = Packages::new(&ResourceRoot::at(resources()), &root, Vec::new());
    let library = places.local_root();
    let mut decided = old("decided", None, None, "/w/decided");
    decided.kind = Some(Kind::Community);
    for desc in [
        old("a-official", Some("github.com/x/a"), None, "/cache/a.miyu"),
        old("b-community", Some("github.com/x/b"), None, "/cache/b.miyu"),
        old(
            "c-friends",
            Some("github.com/x/c"),
            Some("friends"),
            "/cache/c.miyu",
        ),
        old("d-folder", None, None, "/w/d"),
        old("e-backfilled", None, None, ""),
        old("f-moved", Some("github.com/x/f"), None, "/cache/f.miyu"),
        old("g-gone", Some("github.com/x/g"), None, "/cache/g.miyu"),
        decided,
    ] {
        local::write(&library, &desc, &[]).expect("写得进");
    }
    let keys = index(
        &root.official_feed(),
        &[
            entry("a-official", "github.com/x/a", true),
            entry("b-community", "github.com/x/b", false),
            entry("f-moved", "github.com/someone/f", true),
        ],
    );
    let keys = match trusted {
        true => keys,
        false => vec![
            PublicKey::from_base64(&sign::generate().expect("生成得了").public).expect("读得成"),
        ],
    };
    infer(&places, &root.official_feed(), &keys);
    let mut kinds: Vec<(String, Option<Kind>)> = local::ids(&library)
        .into_iter()
        .map(|id| {
            let kind = local::desc(&library, &id).and_then(|desc| desc.kind);
            (id, kind)
        })
        .collect();
    kinds.sort_by(|one, other| one.0.cmp(&other.0));
    kinds
}

#[test]
fn what_can_be_inferred_is_written_back_and_the_rest_stays_unknown() {
    assert_eq!(
        inferred("pk6-infer", true),
        [
            ("a-official".to_string(), Some(Kind::Official)),
            ("b-community".to_string(), Some(Kind::Community)),
            ("c-friends".to_string(), Some(Kind::Community)),
            ("d-folder".to_string(), Some(Kind::File)),
            ("decided".to_string(), Some(Kind::Community)),
            ("e-backfilled".to_string(), None),
            ("f-moved".to_string(), None),
            ("g-gone".to_string(), None),
        ]
    );
}

#[test]
fn an_index_that_fails_its_signature_is_not_used() {
    assert_eq!(
        inferred("pk6-untrusted", false),
        [
            ("a-official".to_string(), None),
            ("b-community".to_string(), None),
            ("c-friends".to_string(), Some(Kind::Community)),
            ("d-folder".to_string(), Some(Kind::File)),
            ("decided".to_string(), Some(Kind::Community)),
            ("e-backfilled".to_string(), None),
            ("f-moved".to_string(), None),
            ("g-gone".to_string(), None),
        ]
    );
}
