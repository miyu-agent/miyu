//! 找脚本（施工 X-12 上）：装了的包里的、自己放的；同一个工具名的自己放的赢；读坏了的清单不算；开头写错的、不是字的跳过、
//! 说为什么；子文件夹、点开头的不算。

use std::fs;

use super::*;
use crate::env::{Env, Platform};
use crate::packages::Packages;
use crate::resources::ResourceRoot;
use crate::test_support::{Scratch, link};

struct Places {
    scratch: Scratch,
    root: DataRoot,
    resources: ResourceRoot,
    admin: AccountId,
}

impl Places {
    fn new() -> Places {
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
        Places {
            resources: ResourceRoot::at(scratch.path().join("res")),
            root: DataRoot::locate(&env).unwrap(),
            admin: AccountId::parse("admin").unwrap(),
            scratch,
        }
    }

    fn write(&self, path: &str, bytes: &[u8]) {
        let path = self.scratch.path().join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    fn found(&self) -> (Vec<Script>, Vec<Skipped>) {
        let packages = Packages::new(&self.resources, &self.root, &self.admin).read();
        find(&packages, &self.root, &self.admin)
    }
}

fn script(description: &str) -> Vec<u8> {
    format!("#!/bin/sh\n# Description: {description}\necho hi\n").into_bytes()
}

#[test]
fn scripts_come_from_installed_packages_and_the_home_folder() {
    let places = Places::new();
    places.write(
        "res/packages/tools/package.toml",
        b"[package]\nprotocol = [1, 1]\nname = { en = \"tools\" }\n\n[builtin]\n",
    );
    places.write(
        "res/packages/tools/scripts/weather.py",
        &script("From the package."),
    );
    places.write(
        "res/packages/tools/scripts/shared.sh",
        &script("From the package."),
    );
    places.write("data/home/admin/scripts/shared", &script("My own."));
    places.write("data/home/admin/scripts/notes", &script("Mine."));
    places.write("data/home/admin/scripts/.hidden", &script("No."));
    places.write("data/home/admin/scripts/nested/inner", &script("No."));
    places.write(
        "data/home/admin/scripts/broken",
        b"#!/bin/sh\necho no header\n",
    );
    places.write("data/home/admin/scripts/binary", &[0xff, 0xfe, 0x00]);
    places.write("res/packages/bad/package.toml", b"[package\n");
    places.write("res/packages/bad/scripts/lost", &script("No."));
    let (scripts, skipped) = places.found();
    let brief: Vec<(&str, &str, Option<&str>)> = scripts
        .iter()
        .map(|found| {
            (
                found.head.name.as_str(),
                found.head.description.as_str(),
                found.package.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        brief,
        [
            ("notes", "Mine.", None),
            ("shared", "My own.", None),
            ("weather", "From the package.", Some("tools")),
        ],
        "照工具名排，同名的自己放的赢"
    );
    assert!(
        scripts[2]
            .file
            .ends_with("res/packages/tools/scripts/weather.py")
    );
    let why: Vec<(String, &str)> = skipped
        .iter()
        .map(|one| {
            (
                one.path.file_name().unwrap().to_string_lossy().into_owned(),
                one.why.as_str(),
            )
        })
        .collect();
    assert_eq!(
        why[..2],
        [
            ("binary".to_string(), "not text"),
            ("broken".to_string(), "Description is missing or empty"),
        ]
    );
    assert_eq!(why[2].0, "shared.sh");
    assert!(
        why[2]
            .1
            .starts_with("script \"shared\" is already provided by "),
        "{why:?}"
    );
    // 被谁盖住（施工 X-13 中）：自己放的那一份；别的跳过的没有。
    let by = skipped[2].by.as_ref().expect("同名的记了被谁盖住");
    assert!(by.ends_with("data/home/admin/scripts/shared"), "{by:?}");
    assert!(skipped[..2].iter().all(|one| one.by.is_none()));
}

/// 两个包带了同一个工具名，出厂的先于家目录（施工 F-8 六补，设计 31 定了的 J），不照编号。
#[test]
fn a_shipped_package_wins_over_a_home_one_whatever_the_ids() {
    let places = Places::new();
    let manifest = |id: &str| {
        format!("[package]\nprotocol = [1, 1]\nname = {{ en = \"{id}\" }}\n\n[builtin]\n")
            .into_bytes()
    };
    places.write("res/packages/z-tools/package.toml", &manifest("z-tools"));
    places.write(
        "res/packages/z-tools/scripts/weather.sh",
        &script("Shipped."),
    );
    places.write(
        "data/home/admin/packages/a-kit/package.toml",
        &manifest("a-kit"),
    );
    places.write(
        "data/home/admin/packages/a-kit/scripts/weather.sh",
        &script("Installed."),
    );
    let (scripts, skipped) = places.found();
    assert_eq!(scripts.len(), 1);
    assert_eq!(scripts[0].package.as_deref(), Some("z-tools"));
    assert_eq!(skipped.len(), 1, "{skipped:?}");
    assert!(
        skipped[0]
            .path
            .ends_with("home/admin/packages/a-kit/scripts/weather.sh")
    );
}

#[test]
fn nothing_anywhere_is_empty() {
    assert_eq!(Places::new().found(), (Vec::new(), Vec::new()));
}

/// 装之前给人看的（施工 X-13 上）：包文件夹 `scripts/` 下开头读得成的工具名，照文件名排；子文件夹、点开头的不算；写错的另
/// 交回为什么（施工 X-13 补）。
#[test]
fn a_package_folder_says_which_scripts_it_carries() {
    let places = Places::new();
    places.write("kit/scripts/weather.py", &script("Weather."));
    places.write("kit/scripts/b-dice.sh", &script("Dice."));
    places.write("kit/scripts/broken.sh", b"#!/bin/sh\necho hi\n");
    places.write("kit/scripts/.hidden.sh", &script("Hidden."));
    places.write("kit/scripts/sub/inner.sh", &script("Inner."));
    let dir = places.scratch.path().join("kit");
    let (names, skipped) = carried(&dir);
    assert_eq!(names, ["b-dice", "weather"]);
    // 读不成的另交回为什么（施工 X-13 补）。
    assert_eq!(skipped.len(), 1, "{skipped:?}");
    assert!(skipped[0].path.ends_with("kit/scripts/broken.sh"));
    assert_eq!(skipped[0].why, "Description is missing or empty");
    assert_eq!(
        carried(&places.scratch.path().join("nothing")),
        (Vec::new(), Vec::new())
    );
}

/// 脚本是链接（施工 X-37）：照它指到的读开头，记链接本身（放行时照它看）；断了的跳过、说为什么；指到目录的不算。
#[test]
fn a_linked_script_is_read_through_and_kept_as_the_link() {
    let places = Places::new();
    places.write("elsewhere/weather.py", &script("Weather."));
    places.write("elsewhere/sub/inner.sh", &script("Inner."));
    let own = own_dir(&places.root, &places.admin);
    fs::create_dir_all(&own).unwrap();
    let (weather, gone) = (own.join("weather.py"), own.join("gone.py"));
    if !(link(
        &places.scratch.path().join("elsewhere/weather.py"),
        &weather,
    ) && link(&places.scratch.path().join("nowhere.py"), &gone)
        && link(
            &places.scratch.path().join("elsewhere/sub"),
            &own.join("sub"),
        ))
    {
        return;
    }
    let (scripts, skipped) = places.found();
    assert_eq!(
        scripts
            .iter()
            .map(|script| (script.head.name.as_str(), script.file.clone()))
            .collect::<Vec<_>>(),
        [("weather", weather)]
    );
    assert_eq!(skipped.len(), 1, "{skipped:?}");
    assert_eq!(skipped[0].path, gone);
    assert!(skipped[0].why.starts_with("broken link: "), "{skipped:?}");
}

/// 状态目录（施工 X-44）：照账号、来处、工具名分，在数据根的 `state/scripts/` 下面。
#[test]
fn a_scripts_state_lives_under_its_account_source_and_name() {
    let root = Path::new("/data");
    let alice = AccountId::parse("alice").expect("合写法");
    assert_eq!(state_root(root), Path::new("/data/state/scripts"));
    assert_eq!(
        state_dir(root, &alice, None, HOME_PACKAGE, "dice").as_deref(),
        Some(Path::new("/data/state/scripts/alice/~home/dice"))
    );
    assert_eq!(
        state_dir(root, &alice, None, "bilibili", "live").as_deref(),
        Some(Path::new("/data/state/scripts/alice/bilibili/live"))
    );
}

/// 列出各账号的状态目录、卸包时删一个来处的（施工 X-44）：照路径排；不是文件夹的不算；删一个来处只删它，别的来处、别的账号的
/// 同名来处都删、没有的不算错。
#[test]
fn states_are_listed_and_one_source_is_forgotten_everywhere() {
    let scratch = Scratch::new();
    let root = scratch.path();
    for dir in [
        "alice/~home/dice",
        "alice/kit/live",
        "bot/kit/watch",
        "bot/other/x",
    ] {
        fs::create_dir_all(state_root(root).join(dir)).expect("建得了");
    }
    fs::write(state_root(root).join("alice/kit/stray"), "x").expect("写得进");
    let listed: Vec<(String, String)> = states(root)
        .into_iter()
        .map(|state| (state.source, state.tool))
        .collect();
    let pair = |source: &str, tool: &str| (source.to_string(), tool.to_string());
    assert_eq!(
        listed,
        [
            pair("kit", "live"),
            pair("~home", "dice"),
            pair("kit", "watch"),
            pair("other", "x"),
        ]
    );
    assert_eq!(
        states(root)[0].path,
        state_root(root).join("alice/kit/live")
    );
    forget_states(root, "kit").expect("删得掉");
    forget_states(root, "nothing").expect("没有的不算错");
    let left: Vec<String> = states(root).into_iter().map(|state| state.tool).collect();
    assert_eq!(left, ["dice", "x"]);
    assert!(states(&root.join("missing")).is_empty());
}

fn venue(text: &str) -> VenueId {
    VenueId::parse(text).expect("场所合写法")
}

/// 场所落到目录名（施工 X-44 补）：可逆；大小写不同的不撞；保留名、末尾的点和空格、中文都转写过；太长的没有；读回时不是唯一
/// 那一种写法的不认。
#[test]
fn a_venue_turns_into_a_folder_name_and_back() {
    for (text, dir) in [
        ("qq:group:123", "@qq_3agroup_3a123"),
        ("QQ", "@_51_51"),
        ("con", "@con"),
        ("a. ", "@a_2e_20"),
        ("群", "@_e7_be_a4"),
        ("x_y", "@x_5fy"),
    ] {
        assert_eq!(venue_dir(&venue(text)).as_deref(), Some(dir), "{text}");
        assert_eq!(venue_of(dir), Some(venue(text)), "{dir}");
    }
    assert_ne!(venue_dir(&venue("A")), venue_dir(&venue("a")));
    assert_eq!(venue_dir(&venue(&"Z".repeat(128))), None, "太长");
    for not in ["qq", "@", "@A", "@_zz", "@_6", "@_61", "@a_2E", "@a.b"] {
        assert_eq!(venue_of(not), None, "{not}");
    }
}

/// 场所会话的状态目录多一层场所（施工 X-44 补）；本机的会话照旧；场所写不成目录名的没有。
#[test]
fn a_venue_session_gets_its_own_state_folder() {
    let root = Path::new("/data");
    let alice = AccountId::parse("alice").expect("合写法");
    let bot = AccountId::parse("bridge").expect("合写法");
    assert_eq!(
        state_dir(root, &alice, Some(&venue("local")), HOME_PACKAGE, "dice").as_deref(),
        Some(Path::new("/data/state/scripts/alice/~home/dice")),
        "本机的会话照旧"
    );
    assert_eq!(
        state_dir(root, &bot, Some(&venue("qq:group:1")), "kit", "live").as_deref(),
        Some(Path::new(
            "/data/state/scripts/bridge/@qq_3agroup_3a1/kit/live"
        ))
    );
    assert_eq!(
        state_dir(root, &bot, Some(&venue(&"Z".repeat(128))), "kit", "live"),
        None
    );
}

/// 列出来、卸包时清（施工 X-44 补）：场所那一层下面的一样认。
#[test]
fn states_under_a_venue_are_listed_and_forgotten_too() {
    let scratch = Scratch::new();
    let root = scratch.path();
    for dir in [
        "bridge/@qq_3agroup_3a1/kit/live",
        "bridge/@qq_3agroup_3a2/kit/live",
        "bridge/@qq_3agroup_3a2/other/x",
        "alice/kit/live",
    ] {
        fs::create_dir_all(state_root(root).join(dir)).expect("建得了");
    }
    let listed: Vec<String> = states(root)
        .into_iter()
        .map(|state| format!("{}/{}", state.source, state.tool))
        .collect();
    assert_eq!(listed, ["kit/live", "kit/live", "kit/live", "other/x"]);
    forget_states(root, "kit").expect("删得掉");
    let left: Vec<String> = states(root).into_iter().map(|state| state.tool).collect();
    assert_eq!(left, ["x"]);
}
