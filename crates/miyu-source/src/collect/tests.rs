//! 收录（施工 G-2 下）：网络换成内存里的一张表。收成一条、多系统、编号被别的仓库占了、拉黑、文件名对不上、一个 Release 里
//! 混了两个包、没有包文件、拿不到；社区来源记一笔；四份规则读得成、写错的报哪个文件。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::*;
use crate::index::Blocked;
use crate::test_support::{Scratch, put};

const API: &str = "https://api.example.invalid";

/// 内存里的网络：地址到字节。
#[derive(Default)]
struct Net(BTreeMap<String, Vec<u8>>);

impl Fetch for Net {
    fn get(&self, url: &str, limit: u64) -> Result<Vec<u8>, String> {
        let bytes = self
            .0
            .get(url)
            .cloned()
            .ok_or_else(|| format!("{url}: 404"))?;
        if bytes.len() as u64 > limit {
            return Err("too big".to_string());
        }
        Ok(bytes)
    }
}

impl Net {
    /// 仓库 `repo` 最新的 Release 带这几个文件：名字、字节。
    fn release(&mut self, repo: &str, files: &[(String, Vec<u8>)]) {
        let assets: Vec<serde_json::Value> = files
            .iter()
            .map(|(name, bytes)| {
                let url = format!("https://github.example.invalid/{repo}/{name}");
                self.0.insert(url.clone(), bytes.clone());
                serde_json::json!({"name": name, "browser_download_url": url, "size": bytes.len()})
            })
            .collect();
        let mut with_text = assets;
        with_text
            .push(serde_json::json!({"name": "notes.txt", "browser_download_url": "x", "size": 1}));
        self.0.insert(
            format!("{API}/repos/{repo}/releases/latest"),
            serde_json::to_vec(&serde_json::json!({"tag_name": "v1", "assets": with_text}))
                .expect("写得成"),
        );
    }
}

/// 打一个包：编号 `id`、版本 `version`，`more` 接在 `[package]` 里；带一个技能、一个人格。交回字节。
fn package(
    root: &Path,
    id: &str,
    version: &str,
    more: &str,
    target: Option<Target>,
) -> (String, Vec<u8>) {
    let folder = root
        .join("src")
        .join(format!(
            "{id}-{version}-{}",
            target.map_or("any", Target::as_str)
        ))
        .join(id);
    put(
        &folder,
        "package.toml",
        format!(
            "[package]\nversion = \"{version}\"\nname = {{ en = \"{id}\", zh = \"包\" }}\n{more}"
        )
        .as_bytes(),
    );
    put(&folder, "skills/sweet/SKILL.md", b"---\nname: sweet\n---\n");
    put(&folder, "skills/draft/README.md", b"not a skill yet");
    put(&folder, "personas/cat/persona.toml", b"[persona]\n");
    let name = archive::file_name(id, version, target);
    let out = root.join("out").join(&name);
    std::fs::create_dir_all(out.parent().expect("有上一层")).expect("建得了");
    archive::pack(&folder, id, &out).expect("打得成");
    (name, std::fs::read(&out).expect("在"))
}

fn rules(official: &[&str]) -> Rules {
    Rules {
        sources: vec![Source::Official],
        official: official.iter().map(|repo| (*repo).to_string()).collect(),
        ..Rules::default()
    }
}

fn scratch_dir(scratch: &Scratch) -> PathBuf {
    let dir = scratch.path().join("work");
    std::fs::create_dir_all(&dir).expect("建得了");
    dir
}

#[test]
fn content_for_other_packages_is_listed_under_provides() {
    let scratch = Scratch::new();
    let mut net = Net::default();
    let (name, bytes) = package(
        scratch.path(),
        "stickers",
        "1.0.0",
        "\n[content.im]\ndir = \"skills\"\n\n[content.memes]\ndir = \"personas\"\n",
        None,
    );
    net.release("miyu-agent/miyu-stickers", &[(name, bytes)]);
    let got = collect(
        &rules(&["miyu-agent/miyu-stickers"]),
        &net,
        API,
        &scratch_dir(&scratch),
        "2026-10-20T03:00:00Z",
    );
    assert_eq!(got.index.packages.len(), 1, "{:?}", got.notes);
    assert_eq!(
        got.index.packages[0].provides["content"],
        ["im", "memes"],
        "给哪几个包带了内容（施工 G-9）"
    );
}

#[test]
fn an_official_release_becomes_an_entry_and_claims_its_id() {
    let scratch = Scratch::new();
    let mut net = Net::default();
    let (name, bytes) = package(scratch.path(), "memes", "1.2.0", "", None);
    net.release("miyu-agent/miyu-memes", &[(name, bytes.clone())]);
    let got = collect(
        &rules(&["miyu-agent/miyu-memes"]),
        &net,
        API,
        &scratch_dir(&scratch),
        "2026-10-19T03:00:00Z",
    );
    assert_eq!(got.index.packages.len(), 1, "{:?}", got.notes);
    let entry = &got.index.packages[0];
    assert_eq!(entry.id, "memes");
    assert_eq!(entry.version, "1.2.0");
    assert_eq!(entry.origin, "github.com/miyu-agent/miyu-memes");
    assert!(entry.official);
    assert_eq!(entry.name["zh"], "包");
    assert_eq!(entry.provides["skills"], ["sweet"]);
    assert_eq!(entry.provides["personas"], ["cat"]);
    assert_eq!(entry.files.len(), 1);
    assert_eq!(entry.files[0].target, None);
    assert_eq!(entry.files[0].size, bytes.len() as u64);
    assert_eq!(entry.files[0].sha256, hex(&Sha256::digest(&bytes)));
    assert_eq!(got.names["memes"], "github.com/miyu-agent/miyu-memes");
    assert!(
        got.notes
            .iter()
            .any(|note| note.contains("claimed id memes")),
        "{:?}",
        got.notes
    );
    assert_eq!(got.index.generated, "2026-10-19T03:00:00Z");
    Index::parse(&serde_json::to_vec(&got.index).expect("写得成")).expect("写出来的读得回去");
}

#[test]
fn one_file_per_target_is_collected_in_order() {
    let scratch = Scratch::new();
    let mut net = Net::default();
    let more = "targets = [\"linux-x86_64\", \"macos-aarch64\"]\n";
    let mac = package(
        scratch.path(),
        "onebot",
        "0.2.0",
        more,
        Some(Target::MacosAarch64),
    );
    let linux = package(
        scratch.path(),
        "onebot",
        "0.2.0",
        more,
        Some(Target::LinuxX86_64),
    );
    net.release("miyu-agent/miyu-onebot", &[mac, linux]);
    let got = collect(
        &rules(&["miyu-agent/miyu-onebot"]),
        &net,
        API,
        &scratch_dir(&scratch),
        "2026-10-19T03:00:00Z",
    );
    let targets: Vec<Option<String>> = got.index.packages[0]
        .files
        .iter()
        .map(|f| f.target.clone())
        .collect();
    assert_eq!(
        targets,
        [
            Some("linux-x86_64".to_string()),
            Some("macos-aarch64".to_string())
        ],
        "{:?}",
        got.notes
    );
}

#[test]
fn taken_blocked_and_broken_releases_are_left_out() {
    let scratch = Scratch::new();
    let mut net = Net::default();
    let (name, bytes) = package(scratch.path(), "memes", "1.0", "", None);
    net.release("someone/memes-copy", &[(name.clone(), bytes.clone())]);
    net.release(
        "miyu-agent/bad-name",
        &[("memes.miyu".to_string(), bytes.clone())],
    );
    let other = package(scratch.path(), "other", "1.0", "", None);
    net.release("miyu-agent/mixed", &[(name, bytes.clone()), other]);
    net.release("miyu-agent/empty", &[]);
    let blocked = package(scratch.path(), "evil", "1.0", "", None);
    net.release("miyu-agent/evil", &[blocked]);
    let mut rules = rules(&[
        "someone/memes-copy",
        "miyu-agent/bad-name",
        "miyu-agent/mixed",
        "miyu-agent/empty",
        "miyu-agent/missing",
        "miyu-agent/evil",
    ]);
    rules
        .sources
        .push(Source::Topic("miyu-package".to_string()));
    rules.names.insert(
        "memes".to_string(),
        "github.com/miyu-agent/miyu-memes".to_string(),
    );
    rules.blocked.push(Blocked {
        id: Some("evil".to_string()),
        origin: None,
        reason: "malware".to_string(),
    });
    let got = collect(
        &rules,
        &net,
        API,
        &scratch_dir(&scratch),
        "2026-10-19T03:00:00Z",
    );
    assert!(got.index.packages.is_empty(), "{:?}", got.index.packages);
    let said = got.notes.join("\n");
    for expected in [
        "someone/memes-copy: skipped, id memes belongs to github.com/miyu-agent/miyu-memes",
        "miyu-agent/bad-name: skipped, memes.miyu should be memes-1.0.miyu",
        "miyu-agent/mixed: skipped, files of different packages in one release",
        "miyu-agent/empty: skipped, no package file in the latest release",
        "miyu-agent/missing: skipped, https://api.example.invalid/repos/miyu-agent/missing/releases/latest: 404",
        "miyu-agent/evil: blocked, malware",
        "topic miyu-package: not collected yet",
    ] {
        assert!(said.contains(expected), "少了「{expected}」：\n{said}");
    }
    assert_eq!(got.index.blocked.len(), 1, "拉黑的随索引下发");
    assert_eq!(
        got.names.get("evil").map(String::as_str),
        Some("github.com/miyu-agent/evil"),
        "拉黑的照样占住编号"
    );
}

#[test]
fn the_rule_files_read_and_names_write_back() {
    let scratch = Scratch::new();
    let dir = scratch.path();
    put(dir, "sources.toml", b"[[source]]\nkind = \"official\"\n\n[[source]]\nkind = \"topic\"\ntopic = \"miyu-package\"\n");
    put(
        dir,
        "official.toml",
        b"repos = [\"miyu-agent/miyu-embed\"]\n",
    );
    put(
        dir,
        "names.toml",
        b"[names]\nembed = \"github.com/miyu-agent/miyu-embed\"\n",
    );
    put(
        dir,
        "blocked.toml",
        b"[[blocked]]\norigin = \"github.com/x/y\"\nreason = \"spam\"\n",
    );
    let rules = Rules::read(dir).expect("读得成");
    assert_eq!(
        rules.sources,
        [Source::Official, Source::Topic("miyu-package".to_string())]
    );
    assert_eq!(rules.official, ["miyu-agent/miyu-embed"]);
    assert_eq!(rules.blocked[0].origin.as_deref(), Some("github.com/x/y"));
    put(
        dir,
        "names.toml",
        rules::names_text(&rules.names).as_bytes(),
    );
    assert_eq!(
        Rules::read(dir).expect("读得成").names,
        rules.names,
        "写回去的读得回来"
    );
    assert_eq!(
        Rules::read(&dir.join("none")).expect("没有的当空的"),
        Rules::default()
    );

    for (file, text, said) in [
        ("official.toml", "repos = [\"../x\"]\n", "official.toml"),
        (
            "sources.toml",
            "[[source]]\nkind = \"npm\"\n",
            "unknown kind",
        ),
        (
            "blocked.toml",
            "[[blocked]]\nreason = \"x\"\n",
            "id or origin",
        ),
        ("names.toml", "[names]\nx = 1\n", "names.x"),
    ] {
        let bad = scratch.path().join(format!("bad-{file}"));
        put(&bad, file, text.as_bytes());
        let error = Rules::read(&bad).expect_err("写错的报出来");
        assert!(error.contains(said), "{file}：{error}");
    }
    assert!(rules::valid_repo("miyu-agent/miyu.embed_2"));
    for bad in ["miyu", "a/b/c", "a/..", "/b", "a b/c"] {
        assert!(!rules::valid_repo(bad), "{bad}");
    }
}

#[test]
fn entries_are_sorted_by_id() {
    let scratch = Scratch::new();
    let mut net = Net::default();
    let zeta = package(scratch.path(), "zeta", "1.0", "", None);
    let alpha = package(scratch.path(), "alpha", "1.0", "", None);
    net.release("miyu-agent/zeta", &[zeta]);
    net.release("miyu-agent/alpha", &[alpha]);
    let got = collect(
        &rules(&["miyu-agent/zeta", "miyu-agent/alpha"]),
        &net,
        API,
        &scratch_dir(&scratch),
        "2026-10-19T03:00:00Z",
    );
    let ids: Vec<&str> = got
        .index
        .packages
        .iter()
        .map(|entry| entry.id.as_str())
        .collect();
    assert_eq!(ids, ["alpha", "zeta"], "{:?}", got.notes);
}
