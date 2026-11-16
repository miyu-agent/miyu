//! 篱笆先剪掉这棵树里碰不到的（施工 V-31）：起点在数据根的上面、里面、旁边、等于它，挡哪些目录和没剪时一样。这里拿没剪的
//! 写法（[`before`]）逐个目录对照，宁可多比不可少挡。施工 PV-RM 起篱笆里只剩数据根；施工 RO-2 起只挡从上面路过的数据根，
//! 起点在数据根里（含等于）的照常走，工作目录不再另算。

use std::path::{Path, PathBuf};

use miyu_tool::Stop;

use super::super::{Fence, files};
use super::Temp;

/// 没剪的 `Fence::from`：起点在数据根里（含等于）的不挡，别的数据根照留，旁边的也留着。
fn before(fence: Fence, root: &Path) -> Fence {
    Fence {
        data_root: fence
            .data_root
            .filter(|fenced| !miyu_fs::within(root, fenced)),
    }
}

fn fence(data_root: Option<&Path>) -> Fence {
    Fence {
        data_root: data_root.map(Path::to_path_buf),
    }
}

/// 只比路径、不碰文件：每种起点下面一批目录（含数据根本身和它里面、大小写换了的写法），剪了的和剪之前的挡法一个不差。大小写
/// 不分的平台（macOS、Windows）上，换了大小写的也算同一处，照样对上。
#[test]
fn trimming_the_fence_keeps_out_the_same_dirs() {
    let base = std::env::temp_dir().join("miyu-fence-paths");
    let home = base.join("home");
    let data = home.join(".miyu");
    let roots = [
        base.clone(),
        home.clone(),
        home.join(".ssh"),
        home.join(".ssh").join("keys"),
        home.join(".config"),
        home.join("proj"),
        home.join("proj").join("src"),
        base.join("HOME"),
        base.join("other"),
        base.join("elsewhere"),
        data.clone(),
        data.join("workspace"),
    ];
    let tails = [
        "a",
        "a/b",
        ".ssh",
        ".ssh/keys",
        ".SSH",
        ".config/gh",
        ".config/gh/hosts",
        "gh",
        "proj",
        "proj/.secret",
        "proj/.secret/z",
        "proj/.Inner",
        ".miyu",
        ".miyu/run",
        ".miyu/workspace",
        ".miyu/workspace/w",
        "workspace/w",
        "run",
        "home/.ssh",
        "HOME/.ssh/k",
        "other/.aws/x",
    ];
    let mut compared = 0;
    for root in &roots {
        for data_root in [None, Some(data.as_path())] {
            let make = || fence(data_root);
            let old = before(make(), root);
            let new = make().from(root);
            let mut dirs: Vec<PathBuf> = tails
                .iter()
                .map(|tail| {
                    tail.split('/')
                        .fold(root.clone(), |dir, part| dir.join(part))
                })
                .collect();
            // 落在起点下面的数据根，照它自己的写法再走进去一层。
            if let Ok(rest) = data.strip_prefix(root) {
                dirs.push(root.join(rest));
                dirs.push(root.join(rest).join("deeper"));
            }
            for dir in &dirs {
                assert_eq!(
                    new.keeps_out(dir),
                    old.keeps_out(dir),
                    "起点 {root:?}，数据根 {data_root:?}，目录 {dir:?}"
                );
                compared += 1;
            }
        }
    }
    assert!(compared > 400, "比了 {compared} 个");
}

/// 剪下来的只剩这棵树里碰得到的：起点旁边的数据根不再每个目录比一遍。
#[test]
fn only_the_reachable_fences_are_left() {
    let base = std::env::temp_dir().join("miyu-fence-left");
    let home = base.join("home");
    let data = base.join("data");
    let left = fence(Some(&data)).from(&home.join("proj"));
    assert_eq!(left.data_root, None, "数据根在旁边");
    let left = fence(Some(&data)).from(&base);
    assert_eq!(left.data_root.as_deref(), Some(data.as_path()));
    let left = fence(Some(&data)).from(&data.join("run"));
    assert_eq!(left.data_root, None, "起点在数据根里：照常走（施工 RO-2）");
    let left = fence(Some(&data)).from(&data);
    assert_eq!(left.data_root, None, "起点就是数据根：照常走");
}

/// 真的走一遍：家目录里别的程序的钥匙照走（施工 PV-RM）；数据根在起点下面的不进，起点在数据根里的照常走（施工 RO-2）。
#[test]
fn walking_from_above_and_inside_the_data_root() {
    let dir = Temp(std::env::temp_dir().join(format!("miyu-walk-fence-{}", std::process::id())));
    for name in [
        "home/notes.txt",
        "home/.ssh/keys/id",
        "home/proj/a.rs",
        "home/.miyu/run/state",
        "home/.miyu/workspace/sub/w.txt",
    ] {
        let path = dir.0.join(name);
        std::fs::create_dir_all(path.parent().expect("有上级目录")).expect("建得了目录");
        std::fs::write(path, "x").expect("写得进");
    }
    let real = |name: &str| std::fs::canonicalize(dir.0.join(name)).expect("在");
    let walk = |root: &str| -> Vec<String> {
        let fence = fence(Some(&real("home/.miyu")));
        let mut names: Vec<String> = files(&real(root), fence, &Stop::default(), |_| true)
            .into_iter()
            .map(|file| file.relative)
            .collect();
        names.sort();
        names
    };
    assert_eq!(
        walk("home"),
        [".ssh/keys/id", "notes.txt", "proj/a.rs"],
        "上面：钥匙照走，路过的数据根不进，连里面的工作区也不进"
    );
    assert_eq!(
        walk("home/.miyu/workspace"),
        ["sub/w.txt"],
        "起点在数据根里：照常走"
    );
    assert_eq!(
        walk("home/.miyu"),
        ["run/state", "workspace/sub/w.txt"],
        "起点就是数据根：照常走"
    );
}
