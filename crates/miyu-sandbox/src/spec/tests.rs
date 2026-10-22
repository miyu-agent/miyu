use std::path::{Path, PathBuf};

use super::*;

/// 蓝图 `sandbox.md` 里的例子：门禁拿它和蓝图里的那一块逐字节比。
fn sample() -> String {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/designs/samples/sandbox/spec.json");
    std::fs::read_to_string(path).expect("读得出")
}

fn paths(list: &[&str]) -> Vec<PathBuf> {
    list.iter().map(PathBuf::from).collect()
}

#[test]
fn the_sample_reads_and_writes_back_byte_for_byte() {
    let sample = sample();
    let spec = Spec::from_json(&sample).expect("读得懂");
    assert_eq!(
        spec,
        Spec {
            write: paths(&["/home/me/project", "/tmp/miyu-sandbox"]),
            hidden: paths(&["/home/me/.miyu"]),
            read: None,
            reveal: Vec::new(),
            mount: false,
            sockets: Vec::new(),
        }
    );
    assert_eq!(spec.to_json().expect("写得成") + "\n", sample);
}

#[test]
fn both_fields_may_be_left_out() {
    let spec = Spec::from_json("{}").expect("读得懂");
    assert!(spec.write.is_empty() && spec.hidden.is_empty());
    assert_eq!(
        spec.to_json().expect("写得成"),
        r#"{"write":[],"hidden":[]}"#
    );
}

#[test]
fn unknown_fields_and_wrong_kinds_are_refused() {
    // 认不得的格不能悄悄跳过：助手不懂的限制，要当规格写坏了。原来的 `readonly`、`network` 也算认不得
    // （2026-09-29 去掉：整盘能读、只管写，不管网络）。`read` 施工 5-12 下为外部身份的会话加回来：只准读名单里的。
    let read = Spec::from_json(r#"{"read":["/usr"]}"#).expect("认得 read");
    assert_eq!(read.read, Some(paths(&["/usr"])));
    for (bad, named) in [
        (r#"{"deny":["/"]}"#, "deny"),
        (r#"{"readonly":["/w/.git"]}"#, "readonly"),
        (r#"{"network":"off"}"#, "network"),
    ] {
        let error = Spec::from_json(bad).expect_err("多了一格");
        assert!(error.to_string().contains(named), "{error}");
    }
    for bad in [r#"{"write":"/usr"}"#, r#"{"hidden":[1]}"#, "not json", ""] {
        assert!(Spec::from_json(bad).is_err(), "{bad}");
    }
}

/// 藏起来的里面单独放开只读的（施工 X-10 中）：没有的不写，规格的字节和以前一样；有的照原样读回来。
#[test]
fn revealed_paths_are_written_only_when_there_are_some() {
    let mut spec = Spec::from_json(r#"{"write":[],"hidden":["/d"]}"#).expect("读得懂");
    assert!(spec.reveal.is_empty());
    assert_eq!(
        spec.to_json().expect("写得成"),
        r#"{"write":[],"hidden":["/d"]}"#
    );
    spec.reveal = paths(&["/d/home/a/skills/notes"]);
    let json = spec.to_json().expect("写得成");
    assert_eq!(
        json,
        r#"{"write":[],"hidden":["/d"],"reveal":["/d/home/a/skills/notes"]}"#
    );
    assert_eq!(Spec::from_json(&json).expect("读得懂"), spec);
}

#[test]
fn a_path_under_a_hidden_one_is_hidden_unless_a_writable_one_brings_it_back() {
    let spec = Spec {
        write: paths(&["/home/a/work", "/home/a/Documents/proj"]),
        hidden: paths(&["/home/a/Documents", "/home/a/work/.secret"]),
        read: None,
        reveal: Vec::new(),
        mount: true,
        sockets: Vec::new(),
    };
    // 私密目录包住了装的位置：看不到。
    assert!(spec.hides(Path::new(
        "/home/a/Documents/miyu/target/debug/miyu-sandbox"
    )));
    assert!(spec.hides(Path::new("/home/a/Documents")));
    // 能写的里面藏着的：照样看不到（能写的先挂，藏的盖在上面）。
    assert!(spec.hides(Path::new("/home/a/work/.secret/bin/miyu")));
    // 藏起来的里面挂回来的能写的：看得到。
    assert!(!spec.hides(Path::new("/home/a/Documents/proj/target/debug/miyu")));
    // 不在藏起来的里面：看得到。
    assert!(!spec.hides(Path::new("/home/a/.local/bin/miyu")));
    assert!(!spec.hides(Path::new("/home/a/Documents2/miyu")));
}
