//! 从软件源装的先后（施工 G-4 上）：依赖在前、装着的不再装、缺的报出来、成环的停得下；挑本机能用的文件。

use serde_json::json;

use super::*;

fn entry(id: &str, depends: &[&str], targets: &[Option<&str>]) -> serde_json::Value {
    let files: Vec<serde_json::Value> = targets
        .iter()
        .map(|target| json!({"target": target, "url": "https://example.invalid/x", "sha256": "ab", "size": 1}))
        .collect();
    json!({"id": id, "version": "1.0", "origin": format!("github.com/a/{id}"), "depends": depends, "files": files})
}

/// 不分系统、版本是 `version` 的一条（施工 PK-2）。
fn versioned(id: &str, version: &str, depends: &[&str]) -> serde_json::Value {
    let mut one = entry(id, depends, &[None]);
    one["version"] = json!(version);
    one
}

/// 装着的：编号、清单写的版本。
fn installed(listed: &[(&str, Option<&str>)]) -> Installed {
    listed
        .iter()
        .map(|(id, version)| (id.to_string(), version.map(str::to_string)))
        .collect()
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
    let none = Installed::new();
    assert_eq!(
        ids(&plan(&index, "app", &none)
            .unwrap_or_else(|refusal| panic!("排得出：{}", refusal.reason))),
        ["base", "core", "lib", "app"]
    );
    let core = installed(&[("core", None)]);
    assert_eq!(
        ids(&plan(&index, "app", &core)
            .unwrap_or_else(|refusal| panic!("排得出：{}", refusal.reason))),
        ["base", "lib", "app"]
    );
    let app = installed(&[("app", None)]);
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
    let none = Installed::new();
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
    let none = Installed::new();
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
            feed("friends", Tier::Added),
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
    let none = Installed::new();
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

/// 版本下限（施工 PK-2）：装着的够的不动；不够的、没装的照先后找第一个版本够的源，一起装（装着的就是一起升）；哪个源都不够的、
/// 没写版本的、排过的那一份不够的报 `depends_missing`，`data.missing` 照写的原样。
#[test]
fn version_floors_pick_new_enough_and_upgrade_along() {
    let indexes = vec![
        (
            feed("friends", Tier::Added),
            index(vec![
                versioned("app", "1.0.0", &["lib>=1.2.0", "base"]),
                versioned("lib", "1.1.0", &[]),
                versioned("base", "0.1.0", &[]),
            ]),
        ),
        (
            feed(OFFICIAL_NAME, Tier::Official),
            index(vec![versioned("lib", "1.3.0", &[])]),
        ),
    ];
    let from = |installed: &Installed| -> Vec<(String, String, String)> {
        plan(&indexes, "app", installed)
            .unwrap_or_else(|refusal| panic!("排得出：{}", refusal.reason))
            .iter()
            .map(|step| {
                let entry = step.entry;
                (
                    entry.id.clone(),
                    entry.version.clone(),
                    step.feed.name.clone(),
                )
            })
            .collect()
    };
    let lib13 = (
        "lib".to_string(),
        "1.3.0".to_string(),
        "official".to_string(),
    );
    assert_eq!(
        from(&Installed::new())[0],
        lib13,
        "自己源里的 1.1.0 不够，照先后挑官方的 1.3.0"
    );
    for old in [Some("1.0.0"), Some("1.2.0-rc.1"), None, Some("r5")] {
        let steps = from(&installed(&[("lib", old), ("base", Some("0.0.1"))]));
        assert_eq!(
            steps.len(),
            2,
            "{old:?}：装着的不够，一起升；base 没写下限，装着就行"
        );
        assert_eq!(steps[0], lib13, "{old:?}");
    }
    for enough in ["1.2.0", "2.0.0"] {
        let steps = from(&installed(&[("lib", Some(enough)), ("base", None)]));
        assert_eq!(steps.len(), 1, "{enough}：装着的够了不动");
    }
    let short = vec![(
        feed(OFFICIAL_NAME, Tier::Official),
        index(vec![
            versioned("app", "1.0.0", &["lib>=2.0.0", "gone>=1.0.0", "base"]),
            versioned("lib", "1.3.0", &[]),
        ]),
    )];
    let refused = plan(&short, "app", &installed(&[("lib", Some("1.0.0"))]))
        .err()
        .expect("不够");
    assert_eq!(refused.reason, "depends_missing");
    assert_eq!(
        refused.data.expect("带 data")["missing"],
        json!(["lib>=2.0.0", "gone>=1.0.0", "base"]),
        "照写的原样、照写的先后"
    );
    let twice = one(index(vec![
        versioned("app", "1.0.0", &["low", "high"]),
        versioned("low", "1.0.0", &["lib"]),
        versioned("high", "1.0.0", &["lib>=2.0.0"]),
        versioned("lib", "1.5.0", &[]),
    ]));
    let refused = plan(&twice, "app", &Installed::new())
        .err()
        .expect("排过的那一份不够");
    assert_eq!(
        refused.data.expect("带 data")["missing"],
        json!(["lib>=2.0.0"])
    );
}
