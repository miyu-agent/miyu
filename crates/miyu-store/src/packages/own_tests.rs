//! 每人自己装的那一层（施工 U23 上下）：账号看得到出厂的、系统那一层、他自己那一层；他那一层读在最后，和系统撞了编号的盖过
//! 系统那一份（施工 U23 下上），系统那一份带程序的不盖、报在他那份上；接在核心手里那一份后面读和整份重读一样；挪不成那一次他
//! 那层就是系统层，不读两遍。

use std::fs;
use std::path::Path;

use miyu_config::package::Code;

use super::*;
use crate::env::{Env, Platform};
use crate::test_support::Scratch;

/// 只带一张 `[package]` 的包。
const KIT: &str = "[package]\nname = { en = \"Kit\" }\n";

/// 一个临时的资源目录和数据根：`res/`、`data/`。
fn places(scratch: &Scratch) -> (ResourceRoot, DataRoot) {
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
    (resources, DataRoot::locate(&env).unwrap())
}

/// 带一个子命令（有程序）的包。
const TOOL: &str = "[package]\nname = { en = \"Tool\" }\nprotocol = [1, 1]\n\n[command]\nname = \"tool\"\nprogram = \"miyu-tool\"\nabout = { en = \"Open tool\" }\n\n[ui]\n";

/// 在 `scratch` 下面 `dir` 里放一个包 `id`。
fn put(scratch: &Scratch, dir: &str, id: &str) {
    put_text(scratch, dir, id, KIT);
}

/// 在 `scratch` 下面 `dir` 里放一个清单写成 `text` 的包 `id`。
fn put_text(scratch: &Scratch, dir: &str, id: &str, text: &str) {
    let path = scratch.path().join(dir).join(id).join(MANIFEST);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// 读到的：编号、层、撞了没有。
fn brief(found: &[Found]) -> Vec<(&str, Layer, Option<Code>)> {
    found
        .iter()
        .map(|one| {
            let code = match &one.read {
                Ok(_) => None,
                Err(Issue::Wrong(problem)) => Some(problem.code),
                Err(Issue::Unreadable(_)) => Some(Code::Syntax),
            };
            (one.id.as_str(), one.layer, code)
        })
        .collect()
}

fn account(name: &str) -> AccountId {
    AccountId::parse(name).unwrap()
}

#[test]
fn an_account_sees_its_own_layer_after_the_system_one() {
    let scratch = Scratch::new();
    let (resources, root) = places(&scratch);
    put(&scratch, "res/packages", "web");
    put(&scratch, "data/system/packages", "kit");
    put(&scratch, "data/home/bob/packages", "kit");
    put(&scratch, "data/home/bob/packages", "notes");
    put(&scratch, "data/home/carol/packages", "theirs");
    let admin = account("alice");
    let bob = Packages::of_account(&resources, &root, &admin, &account("bob"));
    let found = bob.read();
    assert_eq!(
        brief(&found),
        [
            ("kit", Layer::User, None),
            ("notes", Layer::User, None),
            ("web", Layer::Shipped, None),
        ],
        "他自己那层读在最后，同编号的盖过系统那一份；别人那层的看不到"
    );
    assert_eq!(Layer::User.as_str(), "user");
    assert_eq!(
        bob.home_dir(),
        Some(root.system_packages().as_path()),
        "装、卸照旧只动系统那一层"
    );
    let system = Packages::new(&resources, &root, &admin).read();
    assert_eq!(
        brief(&bob.read_over(&system)),
        brief(&found),
        "接在系统那一份后面读和整份重读一样"
    );
    assert_eq!(
        brief(&Packages::new(&resources, &root, &admin).read_over(&system)),
        brief(&system),
        "没有自己那一层的交回系统那一份"
    );
}

#[test]
fn the_owners_own_layer_is_not_read_twice_while_it_is_the_system_one() {
    let scratch = Scratch::new();
    let (resources, root) = places(&scratch);
    put(&scratch, "data/home/alice/packages", "kit");
    let alice = account("alice");
    assert_eq!(
        migrate::system_layer(&root, &alice),
        root.account_dir(&alice).join("packages"),
        "挪不成那一次系统层照旧位置"
    );
    let found = Packages::of_account(&resources, &root, &alice, &alice).read();
    assert_eq!(brief(&found), [("kit", Layer::Home, None)], "只读一遍");
    assert!(Path::new(&found[0].path).starts_with(root.account_dir(&alice)));
}

#[test]
fn an_own_package_overrides_a_shipped_one_but_not_one_that_runs_a_program() {
    let scratch = Scratch::new();
    let (resources, root) = places(&scratch);
    put(&scratch, "res/packages", "web");
    put_text(&scratch, "data/system/packages", "tool", TOOL);
    put(&scratch, "data/home/bob/packages", "web");
    put(&scratch, "data/home/bob/packages", "tool");
    let bob = Packages::of_account(&resources, &root, &account("alice"), &account("bob"));
    assert_eq!(
        brief(&bob.read()),
        [
            ("tool", Layer::Home, None),
            ("tool", Layer::User, Some(Code::Duplicate)),
            ("web", Layer::User, None),
        ],
        "出厂的也盖得过；带程序的系统那一份不盖，报在他那份上"
    );
    assert_eq!(
        bob.own_dir(),
        Some(root.account_dir(&account("bob")).join("packages").as_path())
    );
    assert_eq!(
        Packages::new(&resources, &root, &account("alice")).own_dir(),
        None
    );
}

#[test]
fn what_runs_a_program() {
    let read = |text: &str| package::read_for(text, "0.1.0").unwrap();
    assert!(
        !runs_program(&read(KIT)),
        "只带一张 [package] 的（技能、脚本、预设）不算"
    );
    assert!(runs_program(&read(TOOL)), "子命令");
    let process = TOOL.replace("\n[ui]\n", "\n[process]\n");
    assert!(runs_program(&read(&process)), "核心拉起的扩展");
    let worker =
        "[package]\nname = { en = \"W\" }\nprotocol = [1, 1]\n\n[worker]\nprogram = \"w\"\n";
    assert!(runs_program(&read(worker)), "小程序");
    let remote = "[package]\nname = { en = \"M\" }\n\n[[mcp]]\nname = \"m\"\nurl = \"https://example.com/mcp\"\n";
    assert!(
        runs_program(&read(remote)),
        "网址的 MCP 也不收（成员自己的 MCP 等 MM-1）"
    );
    let theme = "[package]\nname = { en = \"T\" }\n\n[theme]\nfile = \"theme.json\"\n";
    assert!(!runs_program(&read(theme)), "主题");
}
