//! Linux 上用挂载收紧（施工 5-16，`docs/blueprint/sandbox/linux.md`「怎么走」第 3 条）：有能用的 bubblewrap 的机器上，能写的
//! 里面藏着的连名字都看不到、读写不了、挪不走，路上那一层照样能新建、删除；没有的，规格要挂载就拒绝执行。

#![cfg(target_os = "linux")]

mod support;

use miyu_sandbox::EXIT_HELPER;
use support::linux::{bwrap, run, spec, text};
use support::{Dir, serial};

#[test]
fn with_mounts_a_hidden_path_inside_a_writable_one_vanishes_and_nothing_is_frozen() {
    let _serial = serial();
    let work = Dir::new();
    let data = work.path().join("data");
    std::fs::create_dir_all(&data).expect("建得了");
    std::fs::write(data.join("token"), "secret").expect("写得进");
    let mut spec = spec(&[work.path()], &[&data]);
    spec.mount = true;
    let at = |name: &str| work.path().join(name).display().to_string();
    if !bwrap() {
        // 规格要挂载、这台机器没有能用的 bubblewrap：不跑，不悄悄退回 Landlock。
        let out = run(&spec, "echo ran");
        assert_eq!(out.status.code(), Some(i32::from(EXIT_HELPER)), "{out:?}");
        assert!(out.stdout.is_empty(), "没跑");
        return;
    }
    // 施工 5-16：那一层新建、删除都行，藏起来的连名字都看不到，写不进、挪不走。
    let out = run(
        &spec,
        &format!(
            "touch '{new}' && rm '{new}' && ls -A '{data}' | wc -l && echo listed",
            new = at("new"),
            data = at("data")
        ),
    );
    assert_eq!(text(&out.stdout), "0\nlisted\n", "{out:?}");
    let out = run(&spec, &format!("cat '{}'", at("data/token")));
    assert_ne!(out.status.code(), Some(0), "{out:?}");
    let out = run(&spec, &format!("touch '{}'", at("data/x")));
    assert_ne!(out.status.code(), Some(0), "{out:?}");
    let out = run(&spec, &format!("mv '{}' '{}'", at("data"), at("moved")));
    assert_ne!(out.status.code(), Some(0), "{out:?}");
    assert!(data.join("token").exists(), "原样还在");
}
