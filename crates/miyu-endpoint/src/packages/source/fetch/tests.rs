//! 从软件源装的先后（施工 G-4 上）：依赖在前、装着的不再装、缺的报出来、成环的停得下；挑本机能用的文件。

use std::collections::BTreeSet;

use serde_json::json;

use super::*;

fn entry(id: &str, depends: &[&str], targets: &[Option<&str>]) -> serde_json::Value {
    let files: Vec<serde_json::Value> = targets
        .iter()
        .map(|target| json!({"target": target, "url": "https://example.invalid/x", "sha256": "ab", "size": 1}))
        .collect();
    json!({"id": id, "version": "1.0", "origin": format!("github.com/a/{id}"), "depends": depends, "files": files})
}

fn index(entries: Vec<serde_json::Value>) -> Index {
    serde_json::from_value(
        json!({"version": 1, "generated": "2026-10-19T03:00:00Z", "packages": entries}),
    )
    .expect("读得成")
}

/// 只有一个源（官方的）的索引。
fn one(index: Index) -> Indexes {
    vec![(feed(OFFICIAL_NAME, Tier::Official), index)]
}

const OFFICIAL_NAME: &str = "official";

/// 一个源：名字、谁加的，地址、钥匙、目录都空着（排先后用不着）。
fn feed(name: &str, tier: Tier) -> Feed {
    Feed {
        name: name.to_string(),
        tier,
        bases: Vec::new(),
        keys: Vec::new(),
        dir: std::path::PathBuf::new(),
    }
}

fn ids(steps: &[Step<'_>]) -> Vec<String> {
    steps.iter().map(|step| step.entry.id.clone()).collect()
}

#[test]
fn dependencies_come_first_and_installed_ones_are_skipped() {
    let any = [None];
    let index = one(index(vec![
        entry("app", &["lib", "base"], &any),
        entry("lib", &["base", "core"], &any),
        entry("base", &[], &any),
        entry("core", &[], &any),
    ]));
    let none = BTreeSet::new();
    assert_eq!(
        ids(&plan(&index, "app", &none)
            .unwrap_or_else(|refusal| panic!("排得出：{}", refusal.reason))),
        ["base", "core", "lib", "app"]
    );
    let core = BTreeSet::from(["core".to_string()]);
    assert_eq!(
        ids(&plan(&index, "app", &core)
            .unwrap_or_else(|refusal| panic!("排得出：{}", refusal.reason))),
        ["base", "lib", "app"]
    );
    let app = BTreeSet::from(["app".to_string()]);
    assert_eq!(
        ids(&plan(&index, "app", &app)
            .unwrap_or_else(|refusal| panic!("排得出：{}", refusal.reason)))
        .last()
        .map(String::as_str),
        Some("app"),
        "装着的照样重装"
    );
}

#[test]
fn missing_and_unknown_are_refused_and_cycles_stop() {
    let any = [None];
    let index = one(index(vec![
        entry("app", &["gone", "also-gone"], &any),
        entry("a", &["b"], &any),
        entry("b", &["a"], &any),
    ]));
    let none = BTreeSet::new();
    let missing = plan(&index, "app", &none).err().expect("缺依赖");
    assert_eq!(missing.reason, "depends_missing");
    assert_eq!(
        plan(&index, "nothing", &none).err().map(|r| r.reason),
        Some("unknown_package")
    );
    assert_eq!(
        ids(&plan(&index, "a", &none)
            .unwrap_or_else(|refusal| panic!("成环也排得出：{}", refusal.reason))),
        ["b", "a"]
    );
}

#[test]
fn the_file_for_this_system_is_picked() {
    let here = Target::here().map(Target::as_str);
    let other = Target::ALL
        .into_iter()
        .map(Target::as_str)
        .find(|target| Some(*target) != here)
        .expect("有别的");
    let index = one(index(vec![
        entry("both", &[], &[None, here]),
        entry("elsewhere", &[], &[Some(other)]),
    ]));
    let none = BTreeSet::new();
    let steps =
        plan(&index, "both", &none).unwrap_or_else(|refusal| panic!("排得出：{}", refusal.reason));
    assert_eq!(steps[0].file.target.as_deref(), here, "写了本机这一种的先");
    if let Some(here) = here {
        assert_eq!(steps[0].name, format!("both-1.0-{here}.miyu"));
    }
    assert_eq!(
        plan(&index, "elsewhere", &none).err().map(|r| r.reason),
        Some("wrong_target")
    );
}

/// 别的源里的包（施工 U23 下下）：依赖先在它自己的源里找，找不到的照官方的；每一步记着从哪个源下。
#[test]
fn dependencies_fall_back_to_the_official_source() {
    let any = [None];
    let indexes = vec![
        (
            feed("friends", Tier::Personal),
            index(vec![
                entry("app", &["lib", "base"], &any),
                entry("lib", &[], &any),
            ]),
        ),
        (
            feed(OFFICIAL_NAME, Tier::Official),
            index(vec![entry("base", &[], &any), entry("lib", &[], &any)]),
        ),
    ];
    let none = BTreeSet::new();
    let steps =
        plan(&indexes, "app", &none).unwrap_or_else(|refusal| panic!("排得出：{}", refusal.reason));
    let from: Vec<(&str, &str)> = steps
        .iter()
        .map(|step| (step.entry.id.as_str(), step.feed.name.as_str()))
        .collect();
    assert_eq!(
        from,
        [("lib", "friends"), ("base", "official"), ("app", "friends")],
        "自己源里有的照自己的，没有的照官方的"
    );
}
