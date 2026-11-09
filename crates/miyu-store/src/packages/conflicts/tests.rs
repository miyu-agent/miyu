use std::path::Path;

use miyu_config::package;

use super::*;
use crate::layers::Layer as Place;
use crate::packages::Layer;
use crate::test_support::Scratch;

/// 在 `dir` 下造一个包文件夹 `id`：清单 `manifest`，外加 `files`（相对包文件夹的路径、内容）。交回读成了的 [`Found`]。
fn package(dir: &Path, id: &str, manifest: &str, files: &[(&str, &str)]) -> Found {
    let folder = dir.join(id);
    std::fs::create_dir_all(&folder).expect("建得了");
    let path = folder.join("package.toml");
    std::fs::write(&path, manifest).expect("写得进");
    for (file, text) in files {
        let file = folder.join(file);
        std::fs::create_dir_all(file.parent().expect("有上一层")).expect("建得了");
        std::fs::write(file, text).expect("写得进");
    }
    Found {
        id: id.to_string(),
        layer: Layer::Home,
        path,
        read: package::read(manifest).map_err(crate::packages::Issue::Wrong),
    }
}

const BUNDLE: &str = "[package]\nname = { en = \"Kit\" }\n";
const SKILL: &str = "---\nname: pdf\ndescription: Read PDFs.\n---\nbody\n";
const SCRIPT: &str = "#!/bin/sh\n# Description: Echo.\n# Parameters: {\"type\": \"object\", \"properties\": {}}\necho\n";

/// 几层里什么都没有。
fn no_layers(_: Kind, _: &str) -> Vec<Place> {
    Vec::new()
}

/// 撞了谁：包的编号，或者那一层的写法。
fn holder(one: &Conflict) -> String {
    match &one.holder {
        Holder::Package(id) => id.clone(),
        Holder::Layer(layer) => layer.as_str().to_string(),
        Holder::Core => "core".to_string(),
    }
}

fn check(new: &Found, installed: &[Found]) -> Vec<(&'static str, String, String)> {
    let manifest = new.read.as_ref().expect("读成了");
    conflicts(
        &new.id,
        manifest,
        &new.files_dir(),
        installed,
        &[],
        &no_layers,
    )
    .iter()
    .map(|one| (one.kind.as_str(), one.name.clone(), holder(one)))
    .collect()
}

#[test]
fn each_shared_name_is_found_once_per_package_and_kind() {
    let scratch = Scratch::new();
    let dir = scratch.path();
    let tools = "[package]\nprotocol = [1, 1]\nname = { en = \"T\" }\n\n[builtin]\n\n[features.files]\nname = { en = \"F\" }\ntools = [\"read\"]\n";
    let service =
        "[package]\nname = { en = \"S\" }\n\n[[mcp]]\nname = \"lone\"\ncommand = [\"server\"]\n";
    let installed = vec![
        package(dir, "base", tools, &[]),
        package(dir, "svc", service, &[]),
        package(
            dir,
            "kit",
            BUNDLE,
            &[("skills/pdf/SKILL.md", SKILL), ("kb/notes/a.md", "a")],
        ),
    ];
    let new = package(
        &dir.join("new"),
        "mine",
        "[package]\nname = { en = \"M\" }\n\n[[mcp]]\nname = \"lone\"\ncommand = [\"server\"]\n",
        &[
            ("skills/pdf/SKILL.md", SKILL),
            ("scripts/read.sh", SCRIPT),
            ("kb/notes/b.md", "b"),
        ],
    );
    assert_eq!(
        check(&new, &installed),
        vec![
            ("tool", "read".to_string(), "base".to_string()),
            ("skill", "pdf".to_string(), "kit".to_string()),
            ("mcp", "lone".to_string(), "svc".to_string()),
            ("knowledge", "notes".to_string(), "kit".to_string()),
        ]
    );
}

#[test]
fn the_same_package_and_broken_ones_are_not_compared() {
    let scratch = Scratch::new();
    let dir = scratch.path();
    let old = package(dir, "kit", BUNDLE, &[("skills/pdf/SKILL.md", SKILL)]);
    let broken = package(
        dir,
        "broken",
        "[package]\n\n[ui]\n\n[process]\n",
        &[("skills/pdf/SKILL.md", SKILL)],
    );
    let upgrade = package(
        &dir.join("new"),
        "kit",
        BUNDLE,
        &[("skills/pdf/SKILL.md", SKILL)],
    );
    assert!(
        check(&upgrade, &[old, broken]).is_empty(),
        "升级不和自己比，读不成的不算"
    );
}

#[test]
fn a_command_name_and_a_feature_id_are_their_own_kinds() {
    let scratch = Scratch::new();
    let dir = scratch.path();
    let ui = "[package]\nprotocol = [1, 1]\nname = { en = \"U\" }\n\n[command]\nname = \"xcmd\"\nprogram = \"miyu-x\"\nabout = { en = \"X\" }\n\n[ui]\nopens = []\n";
    let feature = "[package]\nprotocol = [1, 1]\nname = { en = \"P\" }\n\n[builtin]\n\n[features.share]\nname = { en = \"S\" }\n";
    let installed = vec![
        package(dir, "zui", ui, &[]),
        package(dir, "zbuilt", feature, &[]),
    ];
    let new_ui = package(&dir.join("a"), "aui", ui, &[]);
    assert_eq!(
        check(&new_ui, &installed),
        vec![("command", "xcmd".to_string(), "zui".to_string())]
    );
    let new_feature = package(&dir.join("b"), "abuilt", feature, &[]);
    assert_eq!(
        check(&new_feature, &installed),
        vec![("feature", "share".to_string(), "zbuilt".to_string())]
    );
}

/// 斜杠命令模板（施工 X-14 上补）：和装着的包的、核心内置的名字、别名比；拥有那条内置命令的包自己不算（装回人格记忆不和
/// `/remember` 撞）。
#[test]
fn slash_commands_meet_installed_packages_and_the_builtins() {
    let scratch = Scratch::new();
    let dir = scratch.path();
    let installed = vec![package(
        dir,
        "kit",
        BUNDLE,
        &[("commands/review.md", "Review.\n")],
    )];
    let reserved = vec![
        ("remember".to_string(), Some("memory".to_string())),
        // 核心自带的（施工 BS-1 下下）：没有包。
        ("reset".to_string(), None),
    ];
    let new = package(
        &dir.join("new"),
        "mine",
        BUNDLE,
        &[
            ("commands/review.md", "Mine.\n"),
            ("commands/reset.md", "Mine.\n"),
            ("commands/fresh.md", "Mine.\n"),
        ],
    );
    let manifest = new.read.as_ref().expect("读成了");
    let found: Vec<(&str, String, String)> = conflicts(
        &new.id,
        manifest,
        &new.files_dir(),
        &installed,
        &reserved,
        &no_layers,
    )
    .iter()
    .map(|one| (one.kind.as_str(), one.name.clone(), holder(one)))
    .collect();
    assert_eq!(
        found,
        vec![
            ("slash", "reset".to_string(), "core".to_string()),
            ("slash", "review".to_string(), "kit".to_string()),
        ]
    );
    let memory = package(
        &dir.join("back"),
        "memory",
        BUNDLE,
        &[("commands/remember.md", "Mine.\n")],
    );
    let manifest = memory.read.as_ref().expect("读成了");
    assert!(
        conflicts(
            &memory.id,
            manifest,
            &memory.files_dir(),
            &[],
            &reserved,
            &no_layers
        )
        .is_empty(),
        "拥有那条内置命令的包自己不算"
    );
}

/// 扩展声明的斜杠命令（施工 X-14 下）和模板一个名字空间：清单里的 `[[slash]]` 撞了装着的包的模板也算。
#[test]
fn declared_slash_commands_count_like_templates() {
    let scratch = Scratch::new();
    let dir = scratch.path();
    let installed = vec![package(
        dir,
        "kit",
        BUNDLE,
        &[("commands/weather.md", "Weather.\n")],
    )];
    let extension = "[package]\nprotocol = [1, 1]\nname = { en = \"W\" }\n\n[command]\nname = \"wx\"\nprogram = \"miyu-wx\"\nabout = { en = \"W\" }\n\n[process]\ncapabilities = [\"commands\"]\n\n[[slash]]\nname = \"weather\"\nsummary = { en = \"Weather\" }\n";
    let new = package(&dir.join("new"), "wx", extension, &[]);
    assert_eq!(
        check(&new, &installed),
        vec![("slash", "weather".to_string(), "kit".to_string())]
    );
}

#[test]
fn page_commands_count_like_slash_commands() {
    let scratch = Scratch::new();
    let dir = scratch.path();
    let installed = vec![package(
        dir,
        "kit",
        BUNDLE,
        &[("commands/weather.md", "Weather.\n")],
    )];
    let extension = "[package]\nprotocol = [1, 1]\nname = { en = \"W\" }\n\n[command]\nname = \"wx\"\nprogram = \"miyu-wx\"\nabout = { en = \"W\" }\n\n[process]\ncapabilities = [\"widgets\"]\n\n[[widget]]\nid = \"week\"\nplace = \"page\"\ntitle = { en = \"Week\" }\ncommand = \"weather\"\n";
    let new = package(&dir.join("new"), "wx", extension, &[]);
    assert_eq!(
        check(&new, &installed),
        vec![("slash", "weather".to_string(), "kit".to_string())]
    );
}

/// 人格、预设（施工 P-11 下）：和装着的别的包带的撞了算，和几层里有的（调的一方交进来）也算，说清是哪一层。
#[test]
fn personas_and_presets_conflict_with_packages_and_layers() {
    let scratch = Scratch::new();
    let dir = scratch.path();
    let kit = package(
        &dir.join("home"),
        "kit",
        BUNDLE,
        &[
            ("personas/cat/persona.toml", "[persona]\n"),
            ("presets/cozy.toml", "[preset]\n"),
        ],
    );
    let new = package(
        &dir.join("new"),
        "pets",
        BUNDLE,
        &[
            ("personas/cat/persona.toml", "[persona]\n"),
            ("personas/dog/persona.toml", "[persona]\n"),
            ("personas/Bad Name/persona.toml", "[persona]\n"),
            ("presets/cozy.toml", "[preset]\n"),
            ("presets/work.toml", "[preset]\n"),
            ("presets/notes.txt", "x"),
        ],
    );
    let layers = |kind: Kind, name: &str| match (kind, name) {
        (Kind::Persona, "dog") => vec![Place::Home],
        (Kind::Preset, "work") => vec![Place::Shipped],
        _ => Vec::new(),
    };
    let manifest = new.read.as_ref().expect("读成了");
    let found: Vec<(&str, String, String)> =
        conflicts(&new.id, manifest, &new.files_dir(), &[kit], &[], &layers)
            .iter()
            .map(|one| (one.kind.as_str(), one.name.clone(), holder(one)))
            .collect();
    assert_eq!(
        found,
        [
            ("persona", "cat".to_string(), "kit".to_string()),
            ("persona", "dog".to_string(), "home".to_string()),
            ("preset", "cozy".to_string(), "kit".to_string()),
            ("preset", "work".to_string(), "shipped".to_string()),
        ]
    );
}

/// 前缀（施工 PN-1，设计 48 第二节第 3 条）：写了的照写的，没写的照包编号；和装着的撞了就是冲突。
#[test]
fn a_prefix_conflicts_with_an_installed_id_or_prefix() {
    let dir = Scratch::new();
    let short = "[package]\nname = { en = \"Kit\" }\nprefix = \"kit\"\n";
    let plain = package(dir.path(), "kit", BUNDLE, &[]);
    let long = package(dir.path(), "miyu-kit", short, &[]);
    assert_eq!(
        check(&long, std::slice::from_ref(&plain)),
        [("prefix", "kit".to_string(), "kit".to_string())],
        "写的前缀撞上装着的包编号"
    );
    assert_eq!(
        check(&plain, std::slice::from_ref(&long)),
        [("prefix", "kit".to_string(), "miyu-kit".to_string())],
        "没写的照包编号，撞上装着的前缀"
    );
    let other = package(dir.path(), "other", BUNDLE, &[]);
    assert!(check(&long, &[other]).is_empty(), "不撞的不报");
}

/// 总线上的名字（施工 BUS-1，设计 67 B3、B6）：两家都 `provides` 撞；一家 `provides`、一家 `plugs` 撞（两个方向）；几家都 `plugs`
/// 不撞；名字不一样的不撞。
#[test]
fn a_bus_name_has_one_provider_and_one_kind() {
    let dir = Scratch::new();
    let voice = |id: &str, export: &str| {
        package(
            dir.path(),
            id,
            &format!(
                "[package]\nprotocol = [1, 1]\nname = {{ en = \"V\" }}\n\n[command]\nname = \"{id}\"\nprogram = \"{id}\"\nabout = {{ en = \"V\" }}\n\n[process]\nstart = \"manual\"\n\n[export]\n{export}\n"
            ),
            &[],
        )
    };
    let stt = voice(
        "stt-a",
        "provides = [\"stt\"]\nmethods = [\"stt.transcribe\"]",
    );
    let other = voice(
        "stt-b",
        "provides = [\"stt\"]\nmethods = [\"stt.transcribe\"]",
    );
    let plug = voice("stt-c", "plugs = [\"stt\"]\nmethods = [\"stt.transcribe\"]");
    let engine = voice(
        "engine-a",
        "plugs = [\"stt.engine\"]\nmethods = [\"stt.engine.transcribe\"]",
    );
    let engine2 = voice(
        "engine-b",
        "plugs = [\"stt.engine\"]\nmethods = [\"stt.engine.transcribe\"]",
    );
    let bus = |found: Vec<(&'static str, String, String)>| -> Vec<(String, String)> {
        found
            .into_iter()
            .filter(|(kind, ..)| *kind == "bus")
            .map(|(_, name, holder)| (name, holder))
            .collect()
    };
    let named = |name: &str, holder: &str| vec![(name.to_string(), holder.to_string())];
    assert_eq!(
        bus(check(&other, std::slice::from_ref(&stt))),
        named("stt", "stt-a"),
        "两家都提供"
    );
    assert_eq!(
        bus(check(&plug, std::slice::from_ref(&stt))),
        named("stt", "stt-a"),
        "提供了的不能接"
    );
    assert_eq!(
        bus(check(&stt, std::slice::from_ref(&plug))),
        named("stt", "stt-c"),
        "接了的不能提供"
    );
    assert!(
        bus(check(&engine2, std::slice::from_ref(&engine))).is_empty(),
        "几家都接不撞"
    );
    assert!(
        bus(check(&engine, std::slice::from_ref(&stt))).is_empty(),
        "名字不一样不撞"
    );
}
