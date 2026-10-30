//! 建目录的测试：一层一层建出来，Unix 上都是 0700；建两次不出错；该是目录的地方是个文件，报错。
//! 同步测不出来：测试里没法断电。

use super::*;
use crate::test_support::Scratch;

#[test]
fn missing_levels_are_made_one_by_one() {
    let scratch = Scratch::bare();
    let deep = scratch.path().join("a").join("b").join("c");
    create_dir_anywhere(&deep).unwrap();
    assert!(deep.is_dir());
    // 建两次也不出错。
    create_dir_anywhere(&deep).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for level in [
            scratch.path().to_path_buf(),
            scratch.path().join("a"),
            scratch.path().join("a").join("b"),
            deep,
        ] {
            let mode = fs::metadata(&level).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o700, "{}", level.display());
        }
    }
}

#[test]
fn a_file_in_the_way_is_an_error() {
    let scratch = Scratch::new();
    create_dir(scratch.path()).unwrap();
    let file = scratch.path().join("a");
    fs::write(&file, "一个文件").unwrap();
    assert!(create_dir(&file).is_err());
    // 挡在中间一层的也一样。
    assert!(create_dir(&file.join("b")).is_err());
    assert_eq!(fs::read_to_string(&file).unwrap(), "一个文件");
}

#[test]
fn only_the_levels_below_the_data_root_are_made() {
    let scratch = Scratch::new();
    let deep = scratch.path().join("index").join("recall");
    create_dir(&deep).unwrap();
    assert!(deep.is_dir());
}

/// 找到标记以后数据根被删掉了（测试的夹具删了它，后台开库的这一下夹在中间，施工 TMP-1 再补）：不连它一起建回来。
#[test]
fn a_data_root_removed_after_it_was_found_is_not_made_again() {
    let scratch = Scratch::bare();
    let root = scratch.path().join("root");
    let deep = root.join("index").join("recall");
    let error = create_below(&root, &deep).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::NotFound, "{error}");
    assert!(!scratch.path().exists(), "什么都没建");
}

/// 认得出的只有写的时候起的那种名字（施工 AS-11）。
#[test]
fn only_names_made_for_writing_count_as_temporary() {
    assert!(is_temp_name(&temp_name("config.toml")));
    for name in [
        ".secrets.toml.4242-0.tmp",
        ".config.toml.1-22.tmp",
        ".a.b.c.9-9.tmp",
    ] {
        assert!(is_temp_name(name), "{name}");
    }
    for name in [
        "config.toml.4242-0.tmp",
        ".config.toml.tmp",
        ".config.toml.4242.tmp",
        ".config.toml.abc-1.tmp",
        ".config.toml.1-x.tmp",
        "..4242-0.tmp",
        ".config.toml.4242-0.tmp.bak",
        "notes.tmp",
    ] {
        assert!(!is_temp_name(name), "{name}");
    }
}

/// 清的时候只删放了够久的、普通文件；刚写的、目录、认不出的都留着（施工 AS-11）。
#[test]
fn sweeping_removes_only_old_temporary_files() {
    use std::time::{Duration, SystemTime};
    let scratch = Scratch::bare();
    let dir = scratch.path();
    fs::create_dir_all(dir).unwrap();
    for name in [
        ".secrets.toml.1-0.tmp",
        ".config.toml.1-1.tmp",
        ".config.toml.tmp",
    ] {
        fs::write(dir.join(name), "x").unwrap();
    }
    fs::create_dir_all(dir.join(".folder.1-2.tmp")).unwrap();
    let later = SystemTime::now() + Duration::from_secs(3600);
    let swept = sweep_temps(dir, later, Duration::from_secs(600)).unwrap();
    let mut removed: Vec<String> = swept
        .removed
        .iter()
        .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    removed.sort();
    assert_eq!(removed, [".config.toml.1-1.tmp", ".secrets.toml.1-0.tmp"]);
    assert!(dir.join(".config.toml.tmp").exists());
    assert!(dir.join(".folder.1-2.tmp").is_dir());
    // 刚写的不碰。
    fs::write(dir.join(".config.toml.1-3.tmp"), "x").unwrap();
    let swept = sweep_temps(dir, SystemTime::now(), Duration::from_secs(600)).unwrap();
    assert!(swept.removed.is_empty());
    // 没有这个目录的不算错。
    assert!(
        sweep_temps(&dir.join("missing"), later, Duration::from_secs(600))
            .unwrap()
            .removed
            .is_empty()
    );
}
