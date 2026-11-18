//! 关起来的会话的规格（施工 AU-001、AU-002）：命令能改的 `.tmp` 不进名单，宽的工作目录退回默认工作区、宽的加进来的目录不给，
//! 数据根怎么都不落进只准读、能写的名单。

use std::path::{Path, PathBuf};

use miyu_kernel::event::{Level, Permission};
use miyu_kernel::id::SessionId;
use miyu_store::root::DataRoot;

use crate::default_workspace::DefaultWorkspace;
use crate::sandbox::Sandbox;

/// 一个临时的家目录，数据根在 `<家>/.miyu`，数据根的 `workspace/` 建好。
struct Scratch {
    base: PathBuf,
    home: PathBuf,
    root: PathBuf,
}

impl Scratch {
    /// 数据根的 `workspace/`。
    fn workspace(&self) -> PathBuf {
        DataRoot::at(self.root.clone()).workspace()
    }
}

impl Scratch {
    fn new(name: &str) -> Scratch {
        let base =
            std::env::temp_dir().join(format!("miyu-confined-{name}-{}", std::process::id()));
        drop(std::fs::remove_dir_all(&base));
        let home = base.join("home");
        let root = home.join(".miyu");
        std::fs::create_dir_all(DataRoot::at(root.clone()).workspace()).expect("建得了数据根");
        std::fs::create_dir_all(root.join("etc/private")).expect("建得了数据根");
        let base = std::fs::canonicalize(&base).expect("换得成真实位置");
        let home = base.join("home");
        let root = home.join(".miyu");
        Scratch { base, home, root }
    }

    /// 关起来的会话的沙盒，默认工作区是会话 `session` 的。
    fn sandbox(&self, session: &SessionId) -> Sandbox {
        Sandbox::new(
            PathBuf::from("miyu-sandbox"),
            Some(self.home.clone()),
            self.root.clone(),
            true,
        )
        .with_workplace(DefaultWorkspace::new(
            &DataRoot::at(self.root.clone()),
            session,
        ))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        drop(std::fs::remove_dir_all(&self.base));
    }
}

fn full() -> Permission {
    Permission {
        level: Level::Full,
        read_only: false,
    }
}

fn session() -> SessionId {
    SessionId::parse("01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91").expect("合编号的写法")
}

/// 名单里有没有一条（照真实的位置）落在数据根里、包住数据根、或者包住家目录。
fn reaches(paths: &[PathBuf], scratch: &Scratch) -> Option<PathBuf> {
    paths
        .iter()
        .find(|path| {
            let real = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
            let workspace = scratch.workspace();
            (real.starts_with(&scratch.root) && !real.starts_with(&workspace))
                || scratch.root.starts_with(&real)
        })
        .cloned()
}

/// 修以前：命令把工作区里的 `.tmp` 删了换成指向数据根的链接，下一条命令的规格把 `.tmp` 原样放进只准读、能写的名单，助手顺着
/// 链接打开，数据根整个能读能写（审计 #001）。现在 `.tmp` 不单独进名单（它在工作区里，工作区已经在），链接换回真的目录。
#[cfg(unix)]
#[test]
fn a_tmp_link_planted_by_a_command_never_opens_the_data_root() {
    let scratch = Scratch::new("tmp-link");
    let workspace = scratch.workspace().join("venue");
    std::fs::create_dir_all(&workspace).expect("建得了工作区");
    std::os::unix::fs::symlink(&scratch.root, workspace.join(".tmp")).expect("建得了链接");
    let sandboxed = scratch
        .sandbox(&session())
        .for_call(&full(), &workspace.to_string_lossy(), &[])
        .expect("写得成")
        .expect("进沙盒");
    let read = sandboxed
        .spec
        .read
        .clone()
        .expect("关起来的写了只准读的名单");
    assert_eq!(reaches(&read, &scratch), None, "只准读的名单碰到了数据根");
    assert_eq!(
        reaches(&sandboxed.spec.write, &scratch),
        None,
        "能写的名单碰到了数据根"
    );
    let tmp = std::fs::symlink_metadata(workspace.join(".tmp")).expect("在");
    assert!(
        tmp.is_dir() && !tmp.file_type().is_symlink(),
        "换回真的目录"
    );
    assert!(
        scratch.root.join("etc/private").is_dir(),
        "链接指到的一样没动"
    );
}

/// `.tmp` 是一个文件（命令写的）：换成目录，照样起得来。
#[test]
fn a_tmp_file_is_replaced_by_a_directory() {
    let scratch = Scratch::new("tmp-file");
    let workspace = scratch.workspace().join("venue");
    std::fs::create_dir_all(&workspace).expect("建得了工作区");
    std::fs::write(workspace.join(".tmp"), "x").expect("写得进");
    let sandboxed = scratch
        .sandbox(&session())
        .for_call(&full(), &workspace.to_string_lossy(), &[])
        .expect("写得成")
        .expect("进沙盒");
    assert!(workspace.join(".tmp").is_dir());
    let env: Vec<_> = sandboxed.env.iter().map(|(key, _)| key.clone()).collect();
    assert!(env.contains(&"TMPDIR".into()));
}

/// 修以前：工作目录是家目录、根目录、数据根里别处的，规格原样把它放进只准读、能写的名单，数据根整个露出来（审计 #002）。
/// 现在照 `miyu_fs::broad` 退回这个会话的默认工作区，`HOME` 跟着指过去。
#[test]
fn a_broad_working_directory_falls_back_to_the_default_workspace() {
    let scratch = Scratch::new("broad");
    let fallback = DataRoot::at(scratch.root.clone()).session_workspace(&session());
    let mut cwds = vec![
        scratch.home.to_string_lossy().into_owned(),
        "~".to_string(),
        scratch.root.join("etc").to_string_lossy().into_owned(),
        scratch.root.to_string_lossy().into_owned(),
    ];
    if cfg!(unix) {
        cwds.push(Path::new("/").to_string_lossy().into_owned());
    }
    for cwd in cwds {
        let sandboxed = scratch
            .sandbox(&session())
            .for_call(&full(), &cwd, &[])
            .expect("写得成")
            .expect("进沙盒");
        let read = sandboxed.spec.read.clone().expect("写了只准读的名单");
        assert_eq!(
            reaches(&read, &scratch),
            None,
            "{cwd}：只准读的碰到了数据根"
        );
        assert_eq!(
            reaches(&sandboxed.spec.write, &scratch),
            None,
            "{cwd}：能写的碰到了数据根"
        );
        let real = std::fs::canonicalize(&fallback).expect("默认工作区建好了");
        assert_eq!(sandboxed.spec.write, vec![real.clone()], "{cwd}");
        let home = sandboxed
            .env
            .iter()
            .find(|(key, _)| key == "HOME")
            .map(|(_, value)| PathBuf::from(value));
        assert_eq!(home, Some(real), "{cwd}");
    }
}

/// 宽的加进来的目录（家目录、包住数据根的）不给；不宽的照给。
#[test]
fn a_broad_extra_directory_is_left_out() {
    let scratch = Scratch::new("dirs");
    let workspace = scratch.workspace().join("venue");
    let shared = scratch.base.join("shared");
    for dir in [&workspace, &shared] {
        std::fs::create_dir_all(dir).expect("建得了目录");
    }
    let dirs = [
        scratch.home.to_string_lossy().into_owned(),
        scratch.base.to_string_lossy().into_owned(),
        shared.to_string_lossy().into_owned(),
    ];
    let sandboxed = scratch
        .sandbox(&session())
        .for_call(&full(), &workspace.to_string_lossy(), &dirs)
        .expect("写得成")
        .expect("进沙盒");
    assert_eq!(sandboxed.spec.write, vec![workspace, shared]);
}

/// 没有默认工作区可退的（造的时候没交）：工作目录宽的不跑，不给宽的。
#[test]
fn a_broad_working_directory_without_a_fallback_is_refused() {
    let scratch = Scratch::new("no-fallback");
    let sandbox = Sandbox::new(
        PathBuf::from("miyu-sandbox"),
        Some(scratch.home.clone()),
        scratch.root.clone(),
        true,
    );
    assert!(
        sandbox
            .for_call(&full(), &scratch.home.to_string_lossy(), &[])
            .is_err()
    );
}
