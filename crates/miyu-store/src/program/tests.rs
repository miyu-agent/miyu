//! 交给程序的环境变量（施工 FH-2下、FH-3下，设计 55 第四节）：新名字和过渡期的旧名字指到同一处；包拿到它在各层的那一格，
//! 拉起前建好、清掉短命暂存、交真实位置。

use std::fs;
use std::path::{Path, PathBuf};

use super::*;
use crate::test_support::Scratch;

/// 名字表 `names` 交出去的名字。
fn names_of(env: &[(&'static str, PathBuf)], value: &Path) -> Vec<&'static str> {
    env.iter()
        .filter(|(_, at)| at == value)
        .map(|(name, _)| *name)
        .collect()
}

#[test]
fn new_names_come_first_and_old_ones_point_at_the_same_place() {
    // 旧名字 FH-6 删：过渡期间一起给，指到同一处。
    assert_eq!(APP, ["MIYU_APP_DIR", "MIYU_PACKAGE_DIR"]);
    assert_eq!(PACKAGE_STATE, ["MIYU_STATE_DIR", "MIYU_PACKAGE_STATE"]);
    assert_eq!(SCRIPT_STATE, ["MIYU_STATE_DIR", "MIYU_SCRIPT_STATE_DIR"]);
    assert_eq!(SCRIPT_CACHE, ["MIYU_CACHE_DIR", "MIYU_SCRIPT_CACHE_DIR"]);
    assert_eq!(SCRIPT_IMAGES, ["MIYU_IMAGES_DIR", "MIYU_SCRIPT_IMAGES_DIR"]);
    assert_eq!(CONFIG, ["MIYU_CONFIG_DIR"]);
    assert_eq!(CACHE, ["MIYU_CACHE_DIR"]);
    assert_eq!(LOG, ["MIYU_LOG_DIR"]);
    assert_eq!(TMP, ["MIYU_TMP_DIR"]);
    assert_eq!(RUNTIME, ["MIYU_RUNTIME_DIR"]);
    assert_eq!(MCP_STATE, ["MIYU_STATE_DIR"]);
}

#[test]
fn each_user_dir_has_its_own_name() {
    for name in crate::root::USER_DIRS {
        let names = user_dir(name).unwrap_or_else(|| panic!("{name} 没有名字"));
        assert_eq!(
            names,
            [format!("MIYU_{}_DIR", name.to_uppercase()).as_str()]
        );
    }
    assert!(user_dir("music").is_none());
}

#[test]
fn a_package_gets_one_folder_in_each_layer() {
    let root = DataRoot::at(PathBuf::from("/data"));
    let dirs = PackageDirs::of(&root, "miyu-im", PathBuf::from("/data/opt/miyu-im"));
    assert_eq!(dirs.app, PathBuf::from("/data/opt/miyu-im"));
    assert_eq!(dirs.config, root.package_config("miyu-im"));
    assert_eq!(dirs.state, root.package_state("miyu-im"));
    assert_eq!(dirs.cache, root.package_cache("miyu-im"));
    assert_eq!(dirs.log, root.package_logs("miyu-im"));
    assert_eq!(dirs.tmp, root.package_tmp("miyu-im"));
    assert_eq!(dirs.runtime, root.package_runtime("miyu-im"));
    let env = dirs.env();
    let expect = [
        (&dirs.app, ["MIYU_APP_DIR", "MIYU_PACKAGE_DIR"].as_slice()),
        (&dirs.config, ["MIYU_CONFIG_DIR"].as_slice()),
        (
            &dirs.state,
            ["MIYU_STATE_DIR", "MIYU_PACKAGE_STATE"].as_slice(),
        ),
        (&dirs.cache, ["MIYU_CACHE_DIR"].as_slice()),
        (&dirs.log, ["MIYU_LOG_DIR"].as_slice()),
        (&dirs.tmp, ["MIYU_TMP_DIR"].as_slice()),
        (&dirs.runtime, ["MIYU_RUNTIME_DIR"].as_slice()),
    ];
    for (at, names) in expect {
        assert_eq!(names_of(&env, at), names, "{}", at.display());
    }
    for name in crate::root::USER_DIRS {
        let at = root.user_dir(name);
        assert_eq!(
            names_of(&env, &at),
            user_dir(name).expect("有名字"),
            "{name}"
        );
    }
    assert_eq!(env.len(), 9 + crate::root::USER_DIRS.len(), "{env:?}");
}

#[test]
fn ready_makes_every_folder_clears_the_short_lived_one_and_hands_real_places() {
    let scratch = Scratch::new();
    let root = DataRoot::at(scratch.path().to_path_buf());
    let app = scratch.path().join("opt/kit");
    fs::create_dir_all(&app).expect("建得了");
    let left = root.package_tmp("kit").join("left.txt");
    fs::create_dir_all(left.parent().expect("有上一级")).expect("建得了");
    fs::write(&left, "上一回的").expect("写得进");
    let kept = root.package_state("kit").join("kept.txt");
    fs::create_dir_all(kept.parent().expect("有上一级")).expect("建得了");
    fs::write(&kept, "留着").expect("写得进");
    let dirs = PackageDirs::ready(&root, "kit", app.clone()).expect("建得了");
    assert!(!left.exists(), "短命暂存拉起前清");
    assert!(kept.is_file(), "状态不动");
    let real = |path: &Path| fs::canonicalize(path).expect("建好了");
    for (made, at) in [
        (&dirs.config, root.package_config("kit")),
        (&dirs.state, root.package_state("kit")),
        (&dirs.cache, root.package_cache("kit")),
        (&dirs.log, root.package_logs("kit")),
        (&dirs.tmp, root.package_tmp("kit")),
        (&dirs.runtime, root.package_runtime("kit")),
    ] {
        assert_eq!(made, &real(&at), "{}", at.display());
    }
    for (_, at) in &dirs.user {
        assert!(at.is_dir(), "{}", at.display());
    }
    assert_eq!(dirs.app, app, "包的文件夹照给的交");
}

#[test]
fn ready_does_not_follow_a_linked_tmp_out_of_the_data_root() {
    let scratch = Scratch::new();
    let elsewhere = Scratch::bare();
    fs::create_dir_all(elsewhere.path().join("kit")).expect("建得了");
    let outside = elsewhere.path().join("kit/precious.txt");
    fs::write(&outside, "别人的").expect("写得进");
    let root = DataRoot::at(scratch.path().to_path_buf());
    if !crate::test_support::link(elsewhere.path(), &scratch.path().join("tmp")) {
        return;
    }
    let made = PackageDirs::ready(&root, "kit", scratch.path().join("opt/kit"));
    assert!(made.is_err(), "tmp/ 是链接的不拉起");
    assert!(outside.is_file(), "链接那头一样都没删");
}

#[test]
fn every_name_in_a_table_points_at_the_same_place() {
    const TWO: Names = &["A", "B"];
    let place = Path::new("/x");
    let pairs: Vec<_> = named(TWO, place).collect();
    assert_eq!(
        pairs,
        [("A", PathBuf::from("/x")), ("B", PathBuf::from("/x"))]
    );
}
