//! `[preset] hidden`（施工 LF-5 上，`presets.md`「格式」）：读得出 `true`、`false`，别的写法报 `not_bool` 写明第几行；上一层写了的
//! 盖下面的，几层都没写的是 `false`；不进指纹、不进快照（不列进选择是头的事，和会话无关）。

use super::*;

#[test]
fn hidden_reads_true_and_false_and_defaults_to_false() {
    assert!(read("[preset]\nhidden = true\n").unwrap().hidden());
    assert!(!read("[preset]\nhidden = false\n").unwrap().hidden());
    assert!(
        !read("[preset]\nname = \"Lean\"\n").unwrap().hidden(),
        "没写的是 false"
    );
    assert!(!PresetFile::default().hidden());
}

#[test]
fn a_hidden_that_is_not_a_switch_names_its_line() {
    for value in ["\"true\"", "\"yes\"", "1", "0", "1.5", "[]"] {
        let problem =
            read(&format!("[preset]\nname = \"Lean\"\nhidden = {value}\n")).expect_err(value);
        assert_eq!(
            (problem.code, problem.line, problem.detail.as_str()),
            (Code::NotBool, Some(3), "preset.hidden"),
            "{value}"
        );
    }
}

#[test]
fn the_upper_layer_s_hidden_covers_the_lower() {
    let shown = read("[preset]\nhidden = false\n").unwrap();
    let hidden = read("[preset]\nhidden = true\n").unwrap();
    let silent = read("[preset]\nname = \"Lean\"\n").unwrap();
    assert!(hidden.clone().over(shown.clone()).hidden());
    assert!(
        !shown.over(hidden.clone()).hidden(),
        "上一层写了 false 的盖住下面的 true"
    );
    assert!(silent.over(hidden).hidden(), "上一层没写的照下面的");
}

/// 写了 `hidden` 和没写的两份造出来的会话一样：指纹、快照里记的预设一字不差（写成哪个值都一样）。
#[test]
fn hidden_leaves_the_digest_and_the_snapshot_alone() {
    let features = installed();
    let plain = read(DEV).unwrap();
    for value in ["true", "false"] {
        let marked =
            read(&DEV.replacen("[preset]\n", &format!("[preset]\nhidden = {value}\n"), 1)).unwrap();
        assert_eq!(marked.hidden(), value == "true");
        assert_eq!(marked.digest(), plain.digest(), "hidden = {value} 不进指纹");
        let made = Chosen::new("dev".to_string(), marked, &features);
        let unmarked = Chosen::new("dev".to_string(), plain.clone(), &features);
        assert_eq!(made.pin(), unmarked.pin(), "hidden = {value} 不进快照");
        assert_eq!(made.off, unmarked.off);
    }
}
