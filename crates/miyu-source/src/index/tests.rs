//! 索引（施工 G-2 上）：读得成、认版本和时刻、不认识的格子略过；旧的换不掉新的；找编号、找拉黑。

use super::*;

const SAMPLE: &str = r#"{
  "version": 1,
  "generated": "2026-10-19T03:00:00Z",
  "packages": [
    {
      "id": "embed",
      "version": "1.0.0",
      "name": {"zh": "语义模型", "en": "Embedding model"},
      "origin": "github.com/miyu-agent/miyu-embed",
      "official": true,
      "provides": {"features": ["embed"]},
      "files": [{"target": null, "url": "https://example.invalid/embed-1.0.0.miyu", "sha256": "ab", "size": 3}],
      "later": "a field from a newer index"
    }
  ],
  "blocked": [{"origin": "github.com/someone/bad", "reason": "malware"}]
}"#;

#[test]
fn an_index_reads_and_ignores_fields_it_does_not_know() {
    let index = Index::parse(SAMPLE.as_bytes()).expect("读得成");
    let embed = index.entry("embed").expect("有");
    assert_eq!(embed.version, "1.0.0");
    assert!(embed.official);
    assert_eq!(embed.files[0].target, None);
    assert_eq!(embed.files[0].size, 3);
    assert_eq!(embed.provides["features"], ["embed"]);
    assert!(embed.summary.is_empty() && embed.depends.is_empty());
    assert!(index.entry("nothing").is_none());
    assert_eq!(
        index
            .blocked("x", "github.com/someone/bad")
            .map(|blocked| blocked.reason.as_str()),
        Some("malware")
    );
    assert!(
        index
            .blocked("embed", "github.com/miyu-agent/miyu-embed")
            .is_none()
    );
}

#[test]
fn a_wrong_version_or_time_is_refused() {
    let newer = SAMPLE.replace("\"version\": 1,", "\"version\": 2,");
    assert_eq!(Index::parse(newer.as_bytes()), Err(Error::Unsupported(2)));
    let time = SAMPLE.replace("2026-10-19T03:00:00Z", "yesterday");
    assert_eq!(
        Index::parse(time.as_bytes()),
        Err(Error::BadTime("yesterday".to_string()))
    );
    assert!(matches!(Index::parse(b"[]"), Err(Error::Malformed(_))));
}

#[test]
fn an_older_index_does_not_replace_a_newer_one() {
    let kept = Index::parse(SAMPLE.as_bytes()).expect("读得成");
    let older = Index {
        generated: "2026-10-18T03:00:00Z".to_string(),
        ..kept.clone()
    };
    let newer = Index {
        generated: "2026-10-20T03:00:00Z".to_string(),
        ..kept.clone()
    };
    assert!(!may_replace(&kept, &older), "旧的不收");
    assert!(may_replace(&kept, &newer));
    assert!(may_replace(&kept, &kept), "同一份重拿照收");
    let broken = Index {
        generated: "?".to_string(),
        ..kept.clone()
    };
    assert!(may_replace(&broken, &kept), "本机那份坏了的照换");
    assert!(!may_replace(&kept, &broken));
}

#[test]
fn a_logo_is_only_fetched_from_where_its_hash_says() {
    let sha = "0123456789abcdef".repeat(4);
    let logo = |path: String, sha256: &str| Logo {
        path,
        sha256: sha256.to_string(),
        size: 1,
    };
    let good = logo(Logo::path_for(&sha, "png"), &sha);
    assert!(good.well_formed());
    assert_eq!(good.version(), "0123456789abcdef");
    assert!(logo(Logo::path_for(&sha, "webp"), &sha).well_formed());
    for (path, sha256) in [
        (Logo::path_for(&sha, "gif"), sha.clone()),
        (format!("../{}", Logo::path_for(&sha, "png")), sha.clone()),
        (format!("{}?x", Logo::path_for(&sha, "png")), sha.clone()),
        (
            Logo::path_for(&sha.to_uppercase(), "png"),
            sha.to_uppercase(),
        ),
        (Logo::path_for("ab", "png"), "ab".to_string()),
    ] {
        assert!(!logo(path.clone(), &sha256).well_formed(), "{path}");
    }
}

#[test]
fn the_four_added_fields_write_and_read_back_and_an_older_index_leaves_them_empty() {
    let old = Index::parse(SAMPLE.as_bytes()).expect("读得成");
    let embed = old.entry("embed").expect("有");
    assert!(embed.recommends.is_empty() && embed.author.is_none());
    assert!(embed.upstream.is_none() && embed.installed.is_none());
    let json = serde_json::to_string(&old).expect("写得成");
    for key in ["recommends", "author", "upstream", "installed"] {
        assert!(!json.contains(key), "{key} 空的不写，老索引原样写出去");
    }

    let mut fresh = old.clone();
    let entry = &mut fresh.packages[0];
    entry.recommends = vec!["memes".to_string(), "voice".to_string()];
    entry.author = Some("Ann".to_string());
    entry.upstream = Some("https://example.invalid/ann/embed".to_string());
    entry.installed = Some(123_456);
    let json = serde_json::to_vec(&fresh).expect("写得成");
    assert_eq!(Index::parse(&json).expect("读得回"), fresh);
    let text = String::from_utf8(json).expect("UTF-8");
    assert!(text.contains("\"installed\":123456") && text.contains("\"recommends\""));
}

/// 施工 AS-2（发布前测试 2026-10-30）：编号、依赖、推荐照包编号的写法，版本只许字母、数字、`.`、`_`、`+`、`-` 且不以 `.`
/// 开头；不合的条目丢掉，别的照读。它们要拼进缓存里的文件名，别人的软件源写的索引不全信：带 `/`、`..`、`\` 的能让下载落到
/// 缓存外面。
#[test]
fn entries_whose_id_or_version_is_not_a_plain_name_are_dropped() {
    let entry = |id: &str, version: &str, depends: &[&str], recommends: &[&str]| {
        serde_json::json!({"id": id, "version": version, "origin": "github.com/someone/x",
            "depends": depends, "recommends": recommends,
            "files": [{"url": "https://example.invalid/x.miyu", "sha256": "ab", "size": 3}]})
    };
    let packages = vec![
        entry("good", "1.0.0-rc.1+build_2", &["lib"], &["extra"]),
        entry("../escape", "1.0", &[], &[]),
        entry("a/b", "1.0", &[], &[]),
        entry("Upper", "1.0", &[], &[]),
        entry("", "1.0", &[], &[]),
        entry("dots", "../../x", &[], &[]),
        entry("back", "1\\..\\..\\x", &[], &[]),
        entry("hidden", ".1", &[], &[]),
        entry("empty", "", &[], &[]),
        entry("colon", "1:0", &[], &[]),
        entry("depends", "1.0", &["../lib"], &[]),
        entry("recommends", "1.0", &[], &["a/b"]),
    ];
    let text = serde_json::json!({"version": 1, "generated": "2026-10-19T03:00:00Z", "packages": packages});
    let index = Index::parse(text.to_string().as_bytes()).expect("读得成");
    let ids: Vec<&str> = index
        .packages
        .iter()
        .map(|entry| entry.id.as_str())
        .collect();
    assert_eq!(ids, ["good"]);
}
