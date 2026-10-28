//! 预设的三层怎么叠（施工 P-2 上）。

use std::fs;

use super::*;
use crate::env::{Env, Platform};
use crate::test_support::Scratch;
use miyu_policy::preset::{Code, Unlisted};

/// 一个临时的资源目录和数据根：`res/`、`data/`。
struct Places {
    scratch: Scratch,
    presets: Presets,
}

impl Places {
    fn new() -> Places {
        let scratch = Scratch::new();
        let resources = ResourceRoot::at(scratch.path().join("res"));
        let env = Env {
            platform: Platform::current(),
            miyu_home: Some(scratch.path().join("data").into_os_string()),
            home: None,
            xdg_cache_home: None,
            local_app_data: None,
            miyu_resources: None,
            exe: None,
        };
        let root = DataRoot::locate(&env).unwrap();
        let admin = AccountId::parse("admin").unwrap();
        // 两个装着的包 `a`、`b`（施工 P-11 上）：先后照交进来的。
        let packages = [scratch.path().join("pkgs/a"), scratch.path().join("pkgs/b")];
        let presets = Presets::new(&resources, &root, &admin).with_packages(&packages);
        Places { scratch, presets }
    }

    /// 一层里的 `presets/` 目录。
    fn dir(&self, layer: Layer) -> PathBuf {
        let base = match layer {
            Layer::Shipped => "res/presets",
            Layer::Package => "pkgs/a/presets",
            Layer::Home => "data/home/admin/presets",
        };
        self.scratch.path().join(base)
    }

    /// 在一层里写一个文件。
    fn write(&self, layer: Layer, file: &str, text: &str) {
        let path = self.dir(layer).join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
}

const DEV: &str = "[preset]\nname = { en = \"Dev\", zh = \"开发\" }\ndefault_persona = \"engineer\"\nunlisted = \"off\"\n\n[software]\nbasesystem = true\nnet = true\n";

#[test]
fn the_layers_stack_field_by_field() {
    let places = Places::new();
    places.write(Layer::Shipped, "dev.toml", DEV);
    places.write(
        Layer::Home,
        "dev.toml",
        "[preset]\nname = { zh = \"我的开发\" }\nunlisted = \"on\"\n\n[software]\nnet = false\n\n[tools]\nshell = false\n",
    );
    let found = places.presets.find("dev").unwrap();
    assert_eq!(found.id, "dev");
    assert_eq!(found.layers, [Layer::Shipped, Layer::Home]);
    assert_eq!(
        found.file.name.as_ref().and_then(|name| name.pick("en")),
        Some("我的开发"),
        "写了名字的一层整格换掉（施工 P-3 补）"
    );
    assert_eq!(found.file.unlisted(), Unlisted::On);
    assert_eq!(found.file.software.get("net"), Some(&false));
    assert_eq!(found.file.software.get("basesystem"), Some(&true));
    assert!(found.file.tools_off.contains("shell"));
}

#[test]
fn only_one_layer_is_enough_and_an_empty_file_counts() {
    let places = Places::new();
    places.write(Layer::Home, "mine.toml", "");
    let found = places.presets.find("mine").unwrap();
    assert_eq!(found.layers, [Layer::Home]);
    assert_eq!(found.file.unlisted(), Unlisted::On);
}

#[test]
fn missing_bad_and_broken_ones_say_why() {
    let places = Places::new();
    assert!(matches!(
        places.presets.find("nope"),
        Err(PresetError::NotFound(_))
    ));
    for id in ["Dev", "../dev", "dev.toml", ""] {
        assert!(
            matches!(places.presets.find(id), Err(PresetError::BadId(_))),
            "{id}"
        );
    }
    places.write(Layer::Shipped, "dev.toml", DEV);
    places.write(
        Layer::Home,
        "dev.toml",
        "[preset]\n\nunlisted = \"maybe\"\n",
    );
    let error = places.presets.find("dev").unwrap_err();
    let PresetError::Invalid(layer, id, problem) = &error else {
        panic!("{error}");
    };
    assert_eq!(
        (*layer, problem.code, problem.line),
        (Layer::Home, Code::BadUnlisted, Some(3))
    );
    assert_eq!(id, "dev");
    assert_eq!(
        error.to_string(),
        "home dev.toml:3: preset.unlisted must be on or off"
    );
    places.write(Layer::Home, "x.toml", "[preset\n");
    let error = places.presets.find("x").unwrap_err();
    assert!(error.to_string().starts_with("home x.toml:"), "{error}");
}

#[test]
fn ids_are_files_named_by_a_valid_id_sorted_and_unique() {
    let places = Places::new();
    places.write(Layer::Shipped, "full.toml", "");
    places.write(Layer::Shipped, "dev.toml", DEV);
    places.write(Layer::Home, "dev.toml", "");
    places.write(Layer::Home, "zed.toml", "");
    places.write(Layer::Home, "Bad.toml", "");
    places.write(Layer::Home, "notes.txt", "");
    fs::create_dir_all(places.dir(Layer::Home).join("dir.toml")).unwrap();
    assert_eq!(places.presets.ids(), ["dev", "full", "zed"]);
    assert!(
        matches!(places.presets.find("dir"), Err(PresetError::NotFound(_))),
        "目录不算"
    );
}

#[test]
fn check_reads_every_layer_and_knows_its_files() {
    let places = Places::new();
    places.write(Layer::Shipped, "dev.toml", "[preset]\nunlisted = 1\n");
    places.write(Layer::Home, "dev.toml", "[colors]\n");
    places.write(Layer::Home, "fine.toml", "");
    let checked = places.presets.check();
    let seen: Vec<(Layer, Code)> = checked
        .iter()
        .map(|checked| match &checked.issue {
            Issue::Wrong(problem) => (checked.layer, problem.code),
            Issue::Unreadable(error) => panic!("{error}"),
        })
        .collect();
    assert_eq!(
        seen,
        [
            (Layer::Shipped, Code::BadUnlisted),
            (Layer::Home, Code::UnknownTable)
        ],
        "上面一层盖住了照样报"
    );
    let file = places.dir(Layer::Shipped).join("dev.toml");
    let one = places.presets.check_file(&file).expect("是预设");
    assert_eq!(one.len(), 1);
    assert_eq!(one[0].layer, Layer::Shipped);
    let fine = places.dir(Layer::Home).join("fine.toml");
    assert!(places.presets.check_file(&fine).expect("是预设").is_empty());
    let missing = places.dir(Layer::Home).join("gone.toml");
    let gone = places.presets.check_file(&missing).expect("位置像预设");
    assert!(matches!(gone[0].issue, Issue::Unreadable(_)));
    for other in [
        places.dir(Layer::Home).join("notes.txt"),
        places.dir(Layer::Home).join("sub").join("a.toml"),
        places.scratch.path().join("data/system/config.toml"),
    ] {
        assert!(places.presets.check_file(&other).is_none(), "{other:?}");
    }
}

#[test]
fn the_shipped_presets_read_cleanly() {
    let resources =
        ResourceRoot::at(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"));
    let scratch = Scratch::new();
    let env = Env {
        platform: Platform::current(),
        miyu_home: Some(scratch.path().join("data").into_os_string()),
        home: None,
        xdg_cache_home: None,
        local_app_data: None,
        miyu_resources: None,
        exe: None,
    };
    let root = DataRoot::locate(&env).unwrap();
    let presets = Presets::new(&resources, &root, &AccountId::parse("admin").unwrap());
    assert!(presets.check().is_empty(), "{:?}", presets.check());
    let full = presets.find("full").unwrap();
    assert_eq!(full.file.unlisted(), Unlisted::On);
    assert!(full.file.software.is_empty());
    let dev = presets.find("dev").unwrap();
    assert_eq!(dev.file.unlisted(), Unlisted::Off);
    assert_eq!(
        dev.file
            .software
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        [
            "authoring",
            "basesystem",
            "env",
            "goal",
            "loop",
            "memory",
            "net",
            "schedule"
        ]
    );
    let minimal = presets.find("minimal").unwrap();
    assert_eq!(minimal.file.unlisted(), Unlisted::Off);
    // 施工 BS-1 上中起只开四个包，不开整个基础系统组：组里以后新加的包照 unlisted 关着。
    assert_eq!(
        minimal
            .file
            .software
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["edit", "read", "shell", "write"]
    );
    assert_eq!(
        minimal
            .file
            .features
            .iter()
            .map(|(id, on)| (id.as_str(), *on))
            .collect::<Vec<_>>(),
        [
            ("env-harness", true),
            ("env-machine", true),
            ("env-model", true),
            ("env-permission", true),
            ("env-profile", true),
            ("env-session", true),
            ("env-time", true),
            ("env-workdir", true),
        ]
    );
    assert_eq!(
        minimal
            .file
            .tools_off
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        Vec::<&str>::new(),
        "施工 BS-1 上中起不用一件件关：glob、grep、trash 是自己的包，照 unlisted 关着"
    );
    // 无功能（施工 P-17）：什么都不写，照 unlisted 全关，以后新装的也关。
    let none = presets.find("none").unwrap();
    assert_eq!(none.file.unlisted(), Unlisted::Off);
    assert!(none.file.software.is_empty() && none.file.features.is_empty());
    assert!(none.file.tools_off.is_empty());
    // 出厂的名字、说明都写三种语言：界面照连接的语言显示（施工 P-3 补）。说明原来不写（施工 P-4 下），项目主人 2026-10-23
    // 定写上（施工 P-16 上）。
    let speaks = |label: &Option<miyu_config::phrases::Label>, language: &str| matches!(label, Some(miyu_config::phrases::Label::Each(phrases)) if phrases.contains_key(language));
    for found in [&full, &dev, &minimal, &none] {
        for language in ["zh", "en", "ja"] {
            assert!(
                speaks(&found.file.name, language),
                "{} {language}",
                found.id
            );
            assert!(
                speaks(&found.file.summary, language),
                "{} {language}",
                found.id
            );
        }
    }
    let zh = |label: &Option<miyu_config::phrases::Label>| {
        label
            .as_ref()
            .and_then(|label| label.pick("zh"))
            .map(str::to_string)
    };
    assert_eq!(
        [&full, &dev, &minimal].map(|found| (zh(&found.file.name), zh(&found.file.summary))),
        [
            (Some("全部功能".into()), Some("开放全部工具。".into())),
            (
                Some("常规开发".into()),
                Some(
                    "在极简开发的基础上增加记忆、网络搜索、子代理、Agent间交流等开发常用工具。"
                        .into()
                )
            ),
            (
                Some("极简开发".into()),
                Some(
                    "只提供Shell和读写文件的工具，最小化工具集以最大程度发挥 AI 原有的能力。"
                        .into()
                )
            ),
        ]
    );
}

/// 每一种问题三种语言都有给人看的一句，`{detail}` 换得进去。
#[test]
fn every_problem_code_is_said_in_three_languages() {
    let resources =
        ResourceRoot::at(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"));
    let codes: Vec<&str> = Code::ALL.iter().map(|code| code.as_str()).collect();
    let unique: std::collections::BTreeSet<&str> = codes.iter().copied().collect();
    assert_eq!(unique.len(), codes.len(), "写法不重复");
    for language in ["zh", "en", "ja"] {
        let human = crate::human::Human::load(&resources, language).unwrap();
        for code in &codes {
            let said = miyu_config::Words::sentence(
                &human,
                &format!("preset-problems/{code}"),
                &[("detail", "X")],
            );
            assert!(
                said.is_some_and(|said| said.contains('X')),
                "{language} 少了 {code}，或者没带 detail"
            );
        }
    }
}

/// 包带的预设在出厂的和家目录的中间，一个编号只认一份（施工 P-11 上，设计 31 第十节第 2、7 条）。
#[test]
fn a_package_preset_sits_between_shipped_and_system() {
    let places = Places::new();
    places.write(
        Layer::Package,
        "chat.toml",
        "[preset]\nname = \"Chat\"\nunlisted = \"off\"\n",
    );
    places.write(Layer::Home, "chat.toml", "[preset]\nunlisted = \"on\"\n");
    let found = places.presets.find("chat").unwrap();
    assert_eq!(found.layers, [Layer::Package, Layer::Home]);
    assert_eq!(found.file.name.unwrap().pick("en"), Some("Chat"));
    places.write(Layer::Shipped, "dev.toml", DEV);
    let b = places.scratch.path().join("pkgs/b/presets");
    fs::create_dir_all(&b).unwrap();
    fs::write(b.join("dev.toml"), "[preset]\nname = \"B\"\n").unwrap();
    fs::write(b.join("chat.toml"), "[preset]\nname = \"B\"\n").unwrap();
    assert_eq!(places.presets.find("dev").unwrap().layers, [Layer::Shipped]);
    assert_eq!(
        places
            .presets
            .find("chat")
            .unwrap()
            .file
            .name
            .unwrap()
            .pick("en"),
        Some("Chat"),
        "排在前面的包赢"
    );
    assert_eq!(
        places.presets.covered(),
        [
            Covered {
                path: b.join("chat.toml"),
                by: places.dir(Layer::Package).join("chat.toml"),
            },
            Covered {
                path: b.join("dev.toml"),
                by: places.dir(Layer::Shipped).join("dev.toml"),
            },
        ]
    );
}
