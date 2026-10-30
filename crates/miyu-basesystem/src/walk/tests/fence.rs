//! 篱笆先剪掉这棵树里碰不到的（施工 V-31）：起点在私密目录、数据根的上面、里面、旁边、等于它，挡哪些目录和剪之前一样。
//! 私密目录不许被搜到是安全边界：这里拿剪之前的写法（[`before`]）逐个目录对照，宁可多比不可少挡。

use std::path::{Path, PathBuf};

use miyu_fs::within;
use miyu_tool::Stop;

use super::super::{Fence, files};
use super::Temp;

/// 剪之前的 `Fence::from`：只去掉包住起点的私密目录，数据根照留。
fn before(mut fence: Fence, root: &Path) -> Fence {
    fence.private.retain(|dir| !within(root, dir));
    fence
}

fn fence(data_root: Option<&Path>, workspace: Option<&Path>, private: &[PathBuf]) -> Fence {
    Fence {
        data_root: data_root.map(Path::to_path_buf),
        workspace: workspace.map(Path::to_path_buf),
        private: private.to_vec(),
    }
}

/// 只比路径、不碰文件：每种起点下面一批目录（含私密目录、数据根本身和它们里面、大小写换了的写法），剪了的和剪之前的挡法
/// 一个不差。大小写不分的平台（macOS、Windows）上，换了大小写的也算同一处，照样对上。
#[test]
fn trimming_the_fence_keeps_out_the_same_dirs() {
    let base = std::env::temp_dir().join("miyu-fence-paths");
    let home = base.join("home");
    let data = home.join(".miyu");
    let private = vec![
        home.join(".ssh"),
        home.join(".config").join("gh"),
        base.join("other").join(".aws"),
        home.join("proj").join(".secret"),
        home.join("proj"),
        base.join("Home").join("proj").join(".Inner"),
    ];
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
                let make = || fence(data_root, workspace.as_deref(), &private);
                let old = before(make(), root);
                let new = make().from(root);
                let mut dirs: Vec<PathBuf> = tails
                    .iter()
                    .map(|tail| {
                        tail.split('/')
                            .fold(root.clone(), |dir, part| dir.join(part))
                    })
                    .collect();
                // 落在起点下面的私密目录、数据根，照它们自己的写法再走进去一层。
                for fenced in private.iter().chain([&data]) {
                    if let Ok(rest) = fenced.strip_prefix(root) {
                        dirs.push(root.join(rest));
                        dirs.push(root.join(rest).join("deeper"));
                    }
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

/// 剪下来的只剩这棵树里碰得到的：起点旁边的私密目录、数据根不再每个目录比一遍。
#[test]
fn only_the_reachable_fences_are_left() {
    let base = std::env::temp_dir().join("miyu-fence-left");
    let home = base.join("home");
    let data = base.join("data");
    let private = vec![
        home.join(".ssh"),
        home.join("proj").join(".secret"),
        base.join("other").join(".aws"),
    ];
    let left = fence(Some(&data), None, &private).from(&home.join("proj"));
    assert_eq!(
        left.private,
        [home.join("proj").join(".secret")],
        "落在起点下面的"
    );
    assert_eq!(left.data_root, None, "数据根在旁边");
    let left = fence(Some(&data), None, &private).from(&base);
    assert_eq!(left.private, private, "起点在上面：都碰得到");
    assert_eq!(left.data_root.as_deref(), Some(data.as_path()));
    let left = fence(Some(&data), None, &private).from(&data.join("run"));
    assert!(left.private.is_empty(), "起点在数据根里：私密目录都碰不到");
    assert_eq!(
        left.data_root.as_deref(),
        Some(data.as_path()),
        "包住起点的数据根照留：它不因为搜的目录在里面就放开"
    );
    let left = fence(None, None, &private).from(&home.join(".ssh"));
    assert!(left.private.is_empty(), "等于起点的私密目录不挡（问过了）");
}

/// 真的走一遍：起点在私密目录上面的不进它、旁边的照走、就是它和在它里面的照走；数据根在起点下面的不进，工作目录那一片照进。
#[test]
fn walking_from_above_inside_beside_and_at_a_private_dir() {
    let dir = Temp(std::env::temp_dir().join(format!("miyu-walk-fence-{}", std::process::id())));
    for name in [
        "home/notes.txt",
        "home/.ssh/keys/id",
        "home/proj/a.rs",
        "home/proj/.secret/token",
        "home/.miyu/run/state",
        "home/.miyu/workspace/sub/w.txt",
    ] {
        let path = dir.0.join(name);
        std::fs::create_dir_all(path.parent().expect("有上级目录")).expect("建得了目录");
        std::fs::write(path, "x").expect("写得进");
    }
    let real = |name: &str| std::fs::canonicalize(dir.0.join(name)).expect("在");
    let private = vec![real("home/.ssh"), real("home/proj/.secret")];
    let walk = |root: &str, workspace: Option<&str>| -> Vec<String> {
        let workspace = workspace.map(real);
        let fence = fence(Some(&real("home/.miyu")), workspace.as_deref(), &private);
        let mut names: Vec<String> = files(&real(root), fence, &Stop::default(), |_| true)
            .into_iter()
            .map(|file| file.relative)
            .collect();
        names.sort();
        names
    };
    assert_eq!(walk("home", None), ["notes.txt", "proj/a.rs"], "上面");
    assert_eq!(
        walk("home", Some("home/.miyu/workspace")),
        ["notes.txt", "proj/a.rs"],
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
    assert_eq!(walk("home/proj", None), ["a.rs"], "旁边的不碰、下面的不进");
    assert_eq!(walk("home/.ssh", None), ["keys/id"], "就是它");
    assert_eq!(walk("home/.ssh/keys", None), ["id"], "在它里面");
    assert_eq!(
        walk("home/.miyu", None),
        Vec::<String>::new(),
        "起点就是数据根：里面的目录照旧不进"
    );
}
