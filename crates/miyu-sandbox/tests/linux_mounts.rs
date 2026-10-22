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

#[test]
fn a_helper_inside_a_hidden_path_still_runs_the_command() {
    // 施工 5-15 下再修：私密目录包住了助手装的位置（核心从 `~/Documents` 下的工作树跑、私密目录加了 `~/Documents`；装在
    // `~/.local/bin` 又把 `~/.local` 设成私密），bwrap 在盖好的里面再起一次助手，原来找不到它，命令全挂。
    let _serial = serial();
    let (work, private) = (Dir::new(), Dir::new());
    let bin = private.path().join("bin");
    std::fs::create_dir_all(&bin).expect("建得了");
    let helper = bin.join("miyu-sandbox");
    std::fs::copy(support::HELPER, &helper).expect("拷得了");
    std::fs::write(private.path().join("token"), "secret").expect("写得进");
    let mut spec = spec(&[work.path()], &[private.path()]);
    spec.mount = true;
    if !bwrap() {
        return;
    }
    let out = std::process::Command::new(&helper)
        .env("LC_ALL", "C")
        .args([
            "run",
            "--spec",
            &spec.to_json().expect("写得成"),
            "--",
            "/bin/sh",
            "-c",
        ])
        .arg(format!(
            "echo ran; cat '{}' 2>/dev/null || echo hidden; ls -A '{}' | wc -l",
            private.path().join("token").display(),
            private.path().display()
        ))
        .stdin(std::process::Stdio::null())
        .output()
        .expect("起得来");
    // 命令照跑；藏起来的照旧看不到，放开的只有助手那一个文件。
    assert_eq!(text(&out.stdout), "ran\nhidden\n1\n", "{out:?}");
}
