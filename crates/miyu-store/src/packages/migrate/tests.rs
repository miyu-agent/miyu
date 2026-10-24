use super::*;
use crate::test_support::Scratch;
use std::path::Path;

/// 一个空的临时目录。
fn temp() -> Scratch {
    let scratch = Scratch::new();
    fs::create_dir_all(scratch.path()).expect("建得了临时目录");
    scratch
}

/// 以前的写法挪成新的：清单进了同名文件夹，原来那个文件夹里的文件照旧在；只有清单、没有文件夹的建一个。
#[test]
fn old_manifests_move_into_their_folders() {
    let home = temp();
    let dir = home.path();
    fs::write(dir.join("relay.toml"), "[package]\n").unwrap();
    fs::create_dir_all(dir.join("relay/page")).unwrap();
    fs::write(dir.join("relay/page/index.html"), "<p>").unwrap();
    fs::write(dir.join("bare.toml"), "[package]\n").unwrap();
    fs::write(dir.join("mermaid.removed"), "").unwrap();
    let moved = old_layout(dir);
    assert_eq!(
        moved
            .iter()
            .map(|m| (m.id.as_str(), m.result.is_ok()))
            .collect::<Vec<_>>(),
        [("bare", true), ("relay", true)]
    );
    assert!(dir.join("relay/package.toml").is_file());
    assert!(
        dir.join("relay/page/index.html").is_file(),
        "原来的文件照旧在"
    );
    assert!(dir.join("bare/package.toml").is_file());
    assert!(!dir.join("relay.toml").exists());
    assert!(dir.join("mermaid.removed").is_file(), "卸掉的那一笔不动");
    assert!(old_layout(dir).is_empty(), "挪过的再来一次什么都不做");
}

/// 文件夹里已经有一份清单的不覆盖，两份都留着；编号不合写法的不动。
#[test]
fn an_existing_manifest_is_not_overwritten() {
    let home = temp();
    let dir = home.path();
    fs::create_dir_all(dir.join("relay")).unwrap();
    fs::write(dir.join("relay/package.toml"), "new").unwrap();
    fs::write(dir.join("relay.toml"), "old").unwrap();
    fs::write(dir.join("Bad.toml"), "x").unwrap();
    let moved = old_layout(dir);
    assert_eq!(moved.len(), 1);
    assert!(matches!(moved[0].result, Err(Unmoved::Both)));
    assert_eq!(
        fs::read_to_string(dir.join("relay/package.toml")).unwrap(),
        "new"
    );
    assert!(dir.join("relay.toml").is_file());
    assert!(dir.join("Bad.toml").is_file());
}

/// 写了 `kind` 的改成照表认的：`kind` 那一行去掉、补上程序表；没写的不在结果里，写的不认识的不动。
#[test]
fn an_old_kind_is_rewritten_into_its_table() {
    let home = temp();
    let dir = home.path();
    fs::create_dir_all(dir.join("relay")).unwrap();
    fs::write(
        dir.join("relay/package.toml"),
        "[package]\nkind = \"ui\"\nname = { en = \"R\" }\n",
    )
    .unwrap();
    fs::create_dir_all(dir.join("fresh")).unwrap();
    fs::write(dir.join("fresh/package.toml"), "[package]\n\n[ui]\n").unwrap();
    fs::create_dir_all(dir.join("odd")).unwrap();
    fs::write(
        dir.join("odd/package.toml"),
        "[package]\nkind = \"daemon\"\n",
    )
    .unwrap();
    let moved = old_kind(dir);
    assert_eq!(
        moved
            .iter()
            .map(|m| (m.id.as_str(), m.result.is_ok()))
            .collect::<Vec<_>>(),
        [("relay", true)]
    );
    assert_eq!(
        fs::read_to_string(dir.join("relay/package.toml")).unwrap(),
        "[package]\nname = { en = \"R\" }\n\n[ui]\n"
    );
    assert!(!dir.join("relay/.package.toml.new").exists());
    assert_eq!(
        fs::read_to_string(dir.join("odd/package.toml")).unwrap(),
        "[package]\nkind = \"daemon\"\n"
    );
    assert!(old_kind(dir).is_empty(), "改过的再来一次什么都不做");
}

/// 一份数据根，在 `scratch` 下面。
fn data_root(scratch: &Scratch) -> DataRoot {
    DataRoot::locate(&crate::env::Env {
        platform: crate::env::Platform::current(),
        miyu_home: Some(scratch.path().join("data").into_os_string()),
        home: None,
        xdg_cache_home: None,
        local_app_data: None,
        miyu_resources: None,
        exe: None,
    })
    .expect("找得到数据根")
}

/// 旧位置放一个包 `kit`、一笔卸掉的出厂的包。
fn old_install(old: &Path) {
    fs::create_dir_all(old.join("kit/skills/notes")).expect("建得了");
    fs::write(old.join("kit/package.toml"), "[package]\n").expect("写得进");
    fs::write(old.join(".removed"), "roleplay\n").expect("写得进");
}

/// 装上的包挪到系统那一层（施工 U23 上上）：旧位置（机主家目录的 `packages/`）有、新位置（`system/packages/`）没有的整个挪过去
/// 一次，里面的东西一样不少，之后照新位置读；旧位置没有的什么都不做。
#[test]
fn installed_packages_move_to_the_system_layer_once() {
    let scratch = temp();
    let root = data_root(&scratch);
    let alice = AccountId::parse("alice").expect("合写法");
    let (old, new) = (old_system_layer(&root, &alice), root.system_packages());
    assert!(
        matches!(to_system_layer(&old, &new), ToSystem::Nothing),
        "旧位置没有的不挪"
    );
    assert_eq!(system_layer(&root, &alice), new, "都没有的照新位置");
    old_install(&old);
    assert_eq!(system_layer(&root, &alice), old, "还没挪的照旧位置读");
    assert!(
        matches!(to_system_layer(&old, &new), ToSystem::Moved),
        "挪了"
    );
    assert!(!old.exists(), "旧位置没了");
    assert_eq!(
        fs::read_to_string(new.join("kit/package.toml")).expect("在"),
        "[package]\n"
    );
    assert!(new.join("kit/skills/notes").is_dir());
    assert_eq!(
        fs::read_to_string(new.join(".removed")).expect("在"),
        "roleplay\n"
    );
    assert_eq!(system_layer(&root, &alice), new, "挪了照新位置");
}

/// 两边都有的（施工 U23 上上）：旧的一样不动，照新位置用。
#[test]
fn both_layers_leave_the_old_one_alone() {
    let scratch = temp();
    let root = data_root(&scratch);
    let alice = AccountId::parse("alice").expect("合写法");
    let (old, new) = (old_system_layer(&root, &alice), root.system_packages());
    old_install(&old);
    fs::create_dir_all(&new).expect("建得了");
    assert!(
        matches!(to_system_layer(&old, &new), ToSystem::Both),
        "两边都有"
    );
    assert!(old.join("kit/package.toml").is_file(), "旧的不动");
    assert!(!new.join("kit").exists(), "不往新的里并");
    assert_eq!(system_layer(&root, &alice), new, "照新位置用");
}

/// 挪不动的（施工 U23 上上，这里拿「`system` 是个文件」顶替权限、跨盘）：旧的原样留着，照旧位置读这一次；下次挪得动了再挪。
#[test]
fn an_unmovable_layer_is_read_where_it_was() {
    let scratch = temp();
    let root = data_root(&scratch);
    let alice = AccountId::parse("alice").expect("合写法");
    let (old, new) = (old_system_layer(&root, &alice), root.system_packages());
    old_install(&old);
    fs::write(root.system(), "not a dir").expect("写得进");
    assert!(
        matches!(to_system_layer(&old, &new), ToSystem::Failed(_)),
        "挪不动"
    );
    assert!(old.join("kit/package.toml").is_file(), "旧的不动");
    assert_eq!(system_layer(&root, &alice), old, "照旧位置读");
    fs::remove_file(root.system()).expect("删得了");
    assert!(
        matches!(to_system_layer(&old, &new), ToSystem::Moved),
        "下次挪了"
    );
}
