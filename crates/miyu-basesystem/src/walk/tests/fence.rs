//! 篱笆先剪掉这棵树里碰不到的（施工 V-31）：起点在数据根的上面、里面、旁边、等于它，挡哪些目录和剪之前一样。数据根不许被
//! 搜到是安全边界：这里拿剪之前的写法（[`before`]）逐个目录对照，宁可多比不可少挡。施工 PV-RM 起篱笆里只剩数据根。

use std::path::{Path, PathBuf};

use miyu_tool::Stop;

use super::super::{Fence, files};
use super::Temp;

/// 剪之前的 `Fence::from`：数据根照留。
fn before(fence: Fence, _root: &Path) -> Fence {
    fence
}

fn fence(data_root: Option<&Path>, workspace: Option<&Path>) -> Fence {
    Fence {
        data_root: data_root.map(Path::to_path_buf),
        workspace: workspace.map(Path::to_path_buf),
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
    let workspaces = [
        None,
        Some(data.join("workspace")),
        Some(home.clone()),
        Some(home.join(".ssh").join("keys")),
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
        for workspace in &workspaces {
            for data_root in [None, Some(data.as_path())] {
                let make = || fence(data_root, workspace.as_deref());
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
                        "起点 {root:?}，工作目录 {workspace:?}，数据根 {data_root:?}，目录 {dir:?}"
                    );
                    compared += 1;
                }
            }
        }
    }
    assert!(compared > 1000, "比了 {compared} 个");
}

/// 剪下来的只剩这棵树里碰得到的：起点旁边的数据根不再每个目录比一遍。
#[test]
fn only_the_reachable_fences_are_left() {
    let base = std::env::temp_dir().join("miyu-fence-left");
    let home = base.join("home");
    let data = base.join("data");
    let left = fence(Some(&data), None).from(&home.join("proj"));
    assert_eq!(left.data_root, None, "数据根在旁边");
    let left = fence(Some(&data), None).from(&base);
    assert_eq!(left.data_root.as_deref(), Some(data.as_path()));
    let left = fence(Some(&data), None).from(&data.join("run"));
    assert_eq!(
        left.data_root.as_deref(),
        Some(data.as_path()),
        "包住起点的数据根照留：它不因为搜的目录在里面就放开"
    );
}

/// 真的走一遍：家目录里别的程序的钥匙照走（施工 PV-RM）；数据根在起点下面的不进，工作目录那一片照进。
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
    let walk = |root: &str, workspace: Option<&str>| -> Vec<String> {
        let workspace = workspace.map(real);
        let fence = fence(Some(&real("home/.miyu")), workspace.as_deref());
        let mut names: Vec<String> = files(&real(root), fence, &Stop::default(), |_| true)
            .into_iter()
            .map(|file| file.relative)
            .collect();
        names.sort();
        names
    };
    assert_eq!(
        walk("home", None),
        [".ssh/keys/id", "notes.txt", "proj/a.rs"],
        "上面：钥匙照走，数据根不进"
    );
    assert_eq!(
        walk("home", Some("home/.miyu/workspace")),
        [".ssh/keys/id", "notes.txt", "proj/a.rs"],
        "从数据根上面搜下来，连工作区也不进"
    );
    assert_eq!(
        walk("home/.miyu/workspace", Some("home/.miyu/workspace")),
        ["sub/w.txt"],
        "工作目录在数据根里：那一片照进"
    );
    assert_eq!(
        walk("home/.miyu/workspace", None),
        Vec::<String>::new(),
        "起点在数据根里、不是工作目录那一片：里面的目录不进"
    );
    assert_eq!(
        walk("home/.miyu", None),
        Vec::<String>::new(),
        "起点就是数据根：里面的目录照旧不进"
    );
}
