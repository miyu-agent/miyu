//! 边界表（施工 4-3 上）：几片重叠时照先后，先对上的算。

use std::path::PathBuf;

use crate::support::Site;
use miyu_fs::{Boundary, Places, Readonly, Zone, within};

#[test]
fn the_workspace_is_writable_but_what_git_runs_outside_is_read_only() {
    let site = Site::new();
    let boundary = site.boundary();
    assert_eq!(boundary.zone(&site.real("work/src/a.rs")), Zone::Writable);
    assert_eq!(boundary.zone(&site.real("work")), Zone::Writable);
    assert_eq!(boundary.zone(&site.real("work/.git/HEAD")), Zone::Writable);
    assert_eq!(
        boundary.zone(&site.real("work/.git/hooks/pre-commit")),
        Zone::Readable
    );
    assert_eq!(boundary.zone(&site.real("work/.git/hooks")), Zone::Readable);
    assert_eq!(
        boundary.zone(&site.real("work/.git/config")),
        Zone::Readable
    );
    assert_eq!(
        boundary.zone(&site.real("work/sub/.git/hooks/x")),
        Zone::Readable,
        "子仓库里的也算"
    );
    // 还不存在的也照位置算：换成真实的位置由 resolve 做，这里直接接上。
    assert_eq!(
        boundary.zone(&site.real("work").join("new.rs")),
        Zone::Writable
    );
}

#[test]
fn nobody_touches_the_data_root_even_inside_the_temp_dir() {
    let site = Site::new();
    let boundary = site.boundary();
    assert_eq!(boundary.zone(&site.real("data/run/token")), Zone::Forbidden);
    assert_eq!(boundary.zone(&site.real("data")), Zone::Forbidden);
    // 临时目录把数据根包在里面（测试、MIYU_HOME 指到那里）：数据根照样谁都不能碰。
    let wide = Boundary::new(&Places {
        temp: site.scratch.0.clone(),
        ..site.places()
    });
    assert_eq!(wide.zone(&site.real("data/run/token")), Zone::Forbidden);
    assert_eq!(wide.zone(&site.real("tmp/t.txt")), Zone::Writable);
}

#[test]
fn a_workspace_moved_into_the_data_root_is_still_a_workspace() {
    let site = Site::new();
    let boundary = Boundary::new(&Places {
        workspace: site.at("data/home/admin/workspace"),
        ..site.places()
    });
    assert_eq!(
        boundary.zone(&site.real("data/home/admin/workspace/w.txt")),
        Zone::Writable
    );
    assert_eq!(boundary.zone(&site.real("data/run/token")), Zone::Forbidden);
}

/// 加进来的目录（施工 5-10 上）：和工作区一样能读能写。
#[test]
fn an_added_dir_is_writable_like_the_workspace() {
    let site = Site::new();
    let boundary = Boundary::new(&Places {
        dirs: vec![site.at("work-other")],
        ..site.places()
    });
    assert_eq!(
        boundary.zone(&site.real("work-other/x.txt")),
        Zone::Writable
    );
    assert_eq!(boundary.zone(&site.real("work-other")), Zone::Writable);
    assert_eq!(
        boundary.zone(&site.real("home/notes.txt")),
        Zone::Outside,
        "别处照旧"
    );
}

#[test]
fn inside_an_added_dir_what_git_runs_outside_is_read_only() {
    let site = Site::new();
    let boundary = Boundary::new(&Places {
        workspace: site.at("work-other"),
        dirs: vec![site.at("work")],
        ..site.places()
    });
    assert_eq!(boundary.zone(&site.real("work/src/a.rs")), Zone::Writable);
    assert_eq!(
        boundary.zone(&site.real("work/.git/hooks/pre-commit")),
        Zone::Readable
    );
    assert_eq!(
        boundary.zone(&site.real("work/.git/config")),
        Zone::Readable
    );
}

/// 加进来的目录落进了数据根（例如报来以后被换成了链接）：数据根照样谁都不能碰。
#[test]
fn the_data_root_wins_over_an_added_dir() {
    let site = Site::new();
    let boundary = Boundary::new(&Places {
        dirs: vec![site.at("data"), site.at("data/home")],
        ..site.places()
    });
    assert_eq!(boundary.zone(&site.real("data/run/token")), Zone::Forbidden);
    assert_eq!(
        boundary.zone(&site.real("data/home/admin/workspace/w.txt")),
        Zone::Forbidden
    );
}

#[test]
fn an_added_dir_that_does_not_exist_is_left_out() {
    let site = Site::new();
    let boundary = Boundary::new(&Places {
        dirs: vec![site.at("nowhere")],
        ..site.places()
    });
    assert_eq!(
        boundary.zone(&site.real("").join("nowhere/x.txt")),
        Zone::Outside
    );
}

#[test]
fn temp_is_writable_system_and_toolchains_read_only_the_rest_outside() {
    let site = Site::new();
    let boundary = site.boundary();
    assert_eq!(boundary.zone(&site.real("tmp/t.txt")), Zone::Writable);
    assert_eq!(boundary.zone(&site.real("sys/lib.txt")), Zone::Readable);
    assert_eq!(
        boundary.zone(&site.real("home/.cargo/registry/x.rs")),
        Zone::Readable
    );
    assert_eq!(boundary.zone(&site.real("home/.ssh/id")), Zone::Outside);
    assert_eq!(boundary.zone(&site.real("home/notes.txt")), Zone::Outside);
    // 一段一段比：work-other 不在 work 里。
    assert_eq!(boundary.zone(&site.real("work-other/x.txt")), Zone::Outside);
}

#[test]
fn a_shelf_in_the_data_root_is_its_own_zone() {
    let site = Site::new();
    let boundary = Boundary::new(&Places {
        shelves: vec![site.at("data/home/admin/skills/notes"), site.at("nowhere")],
        ..site.places()
    });
    let skill = site.real("data/home/admin/skills/notes/SKILL.md");
    assert_eq!(boundary.zone(&skill), Zone::Shelf);
    assert_eq!(
        boundary.zone(&site.real("data/home/admin/skills/notes")),
        Zone::Shelf
    );
    // 旁边的照旧谁都不能碰。
    assert_eq!(
        boundary.zone(&site.real("data/home/admin/skills")),
        Zone::Forbidden
    );
    assert_eq!(boundary.zone(&site.real("data/run/token")), Zone::Forbidden);
}

#[test]
fn a_place_that_does_not_exist_is_left_out() {
    let site = Site::new();
    let boundary = Boundary::new(&Places {
        readable: vec![site.at("nowhere"), site.at("sys")],
        ..site.places()
    });
    assert_eq!(boundary.zone(&site.real("sys/lib.txt")), Zone::Readable);
    assert_eq!(boundary.zone(&site.real("home/notes.txt")), Zone::Outside);
}

#[test]
fn the_data_root_is_known_in_any_case_where_case_does_not_matter() {
    let site = Site::new();
    let boundary = site.boundary();
    let token = site.real("data/run/token");
    let shouting = PathBuf::from(token.to_string_lossy().to_uppercase());
    let expected = if cfg!(any(target_os = "macos", windows)) {
        Zone::Forbidden
    } else {
        // 分大小写的文件系统上，那是另一个地方。
        Zone::Outside
    };
    assert_eq!(boundary.zone(&shouting), expected, "{}", shouting.display());
}

#[test]
fn within_counts_the_dir_itself_and_folds_case_where_case_does_not_matter() {
    let site = Site::new();
    let data = site.real("data");
    assert!(within(&data, &data), "自己也算");
    assert!(within(&site.real("data/run/token"), &data));
    assert!(!within(&site.real("work"), &data));
    // 只是名字开头一样的兄弟目录不算：照一段一段比，不照字符串比。
    let sibling = PathBuf::from(format!("{}-old", data.display()));
    assert!(!within(&sibling, &data), "{}", sibling.display());
    let shouting = PathBuf::from(data.to_string_lossy().to_uppercase()).join("x");
    assert_eq!(
        within(&shouting, &data),
        cfg!(any(target_os = "macos", windows)),
        "{}",
        shouting.display()
    );
}

#[test]
fn this_machine_has_its_system_and_temp_dirs() {
    let site = Site::new();
    let places = Places::here(site.at("work"), site.at("data"), Some(&site.at("home")));
    // 家目录下的工具链目录都列上了。场地本身在临时目录里，所以不拿它们查是哪一片。
    for name in [".cargo", ".rustup", ".npm", ".gitconfig"] {
        assert!(
            places.readable.contains(&site.at("home").join(name)),
            "{name}"
        );
    }
    let boundary = Boundary::new(&places);
    let temp = std::fs::canonicalize(std::env::temp_dir()).expect("临时目录在");
    assert_eq!(boundary.zone(&temp), Zone::Writable);
    let system = if cfg!(windows) {
        PathBuf::from(std::env::var_os("SystemRoot").expect("Windows 上有 SystemRoot"))
    } else {
        PathBuf::from("/usr")
    };
    let system = std::fs::canonicalize(system).expect("系统目录在");
    assert_eq!(boundary.zone(&system), Zone::Readable);
}

#[test]
fn private_dirs_are_their_own_zone_after_the_data_root_and_added_dirs() {
    let site = Site::new();
    let places = Places {
        private: vec![
            site.at("home/.ssh"),
            site.at("data/run"),
            site.at("work-other"),
        ],
        dirs: vec![site.at("work-other")],
        ..site.places()
    };
    let boundary = Boundary::new(&places);
    assert_eq!(boundary.zone(&site.real("home/.ssh/id")), Zone::Private);
    assert_eq!(boundary.zone(&site.real("home/.ssh")), Zone::Private);
    assert_eq!(boundary.zone(&site.real("home/notes.txt")), Zone::Outside);
    assert_eq!(
        boundary.zone(&site.real("data/run/token")),
        Zone::Forbidden,
        "数据根在前"
    );
    assert_eq!(
        boundary.zone(&site.real("work-other/x.txt")),
        Zone::Writable,
        "人加进来的目录在前"
    );
}

#[test]
fn the_private_list_hangs_off_the_home() {
    let home = PathBuf::from("/h");
    let private = miyu_fs::private_dirs(&home);
    for name in [".ssh", ".gnupg", ".claude", ".miyu"] {
        assert!(private.contains(&home.join(name)), "{name}");
    }
    assert!(private.contains(&home.join(".config").join("gh")));
    // 这个平台的浏览器配置、钥匙串也在。
    let here = if cfg!(target_os = "macos") {
        home.join("Library").join("Keychains")
    } else if cfg!(windows) {
        home.join("AppData").join("Roaming").join("GitHub CLI")
    } else {
        home.join(".mozilla")
    };
    assert!(private.contains(&here), "{}", here.display());
    let places = Places::here(PathBuf::from("/w"), PathBuf::from("/d"), Some(&home));
    assert_eq!(places.private, private, "照家目录拼上");
    let nowhere = Places::here(PathBuf::from("/w"), PathBuf::from("/d"), None);
    assert!(nowhere.private.is_empty(), "不知道家目录的没有");
}

/// 只禁写的名单（施工 5-17 下）：能写的那几片里落在名单上的只能读，相对的在工作区、加进来的目录里任一层都算；去掉的照能写；
/// 不碰读：名单上的在能写的以外照原来那一片算。还不存在的也照位置算。
#[test]
fn the_readonly_list_turns_writable_places_read_only() {
    let site = Site::new();
    crate::support::file(&site.at("work/Makefile"), "all:");
    crate::support::file(&site.at("work/sub/.vscode/tasks.json"), "{}");
    crate::support::file(&site.at("work-other/.envrc"), "export A=1");
    crate::support::file(&site.at("tmp/hook.sh"), "#!/bin/sh");
    let list = Readonly::with(
        Some(&site.at("home")),
        &[
            "Makefile".to_string(),
            site.at("tmp/hook.sh").to_string_lossy().into_owned(),
            "~/notes.txt".to_string(),
        ],
        &[".git/config".to_string()],
    );
    let boundary = Boundary::new(&Places {
        dirs: vec![site.at("work-other")],
        readonly: list.clone(),
        ..site.places()
    });
    assert_eq!(boundary.zone(&site.real("work/Makefile")), Zone::Readable);
    assert_eq!(
        boundary.zone(&site.real("work/sub/.vscode/tasks.json")),
        Zone::Readable,
        "任一层"
    );
    assert_eq!(
        boundary.zone(&site.real("work/.git/hooks/pre-commit")),
        Zone::Readable
    );
    assert_eq!(
        boundary.zone(&site.real("work/.git/config")),
        Zone::Writable,
        "去掉了"
    );
    assert_eq!(
        boundary.zone(&site.real("work-other/.envrc")),
        Zone::Readable,
        "加进来的目录里"
    );
    assert_eq!(boundary.zone(&site.real("tmp/hook.sh")), Zone::Readable);
    assert_eq!(boundary.zone(&site.real("tmp/t.txt")), Zone::Writable);
    assert_eq!(
        boundary.zone(&site.real("home/notes.txt")),
        Zone::Outside,
        "能写的以外照原来那一片：读不放开"
    );
    // 工作区就是家目录：家目录下的那几项只能读，还没有的也算。
    let home = Boundary::new(&Places {
        workspace: site.at("home"),
        readonly: list,
        ..site.places()
    });
    assert_eq!(home.zone(&site.real("home/notes.txt")), Zone::Readable);
    assert_eq!(
        home.zone(&site.real("home").join("todo.txt")),
        Zone::Writable
    );
    let shipped = Boundary::new(&Places {
        workspace: site.at("home"),
        ..site.places()
    });
    assert_eq!(
        shipped.zone(&site.real("home").join(".gitconfig")),
        Zone::Readable,
        "出厂的，还不存在"
    );
}

/// 作为文件的 `.git`（worktree、子模块）：它说仓库在哪，钩子、配置就在哪，`.git/config` 在名单上时它也只能读（施工 5-17 下）。
#[test]
fn a_git_file_is_read_only_with_the_git_config() {
    let site = Site::new();
    crate::support::file(&site.at("work/tree/.git"), "gitdir: ../.git/worktrees/tree");
    let boundary = site.boundary();
    assert_eq!(boundary.zone(&site.real("work/tree/.git")), Zone::Readable);
    assert_eq!(
        boundary.zone(&site.real("work/.git")),
        Zone::Writable,
        "是目录的照里面那几样算"
    );
    let without = Boundary::new(&Places {
        readonly: Readonly::with(Some(&site.at("home")), &[], &[".git/config".to_string()]),
        ..site.places()
    });
    assert_eq!(without.zone(&site.real("work/tree/.git")), Zone::Writable);
}

/// 大小写不分的平台上，换个大小写还是同一处（施工 4-3 上起照这个比）。
#[cfg(any(target_os = "macos", windows))]
#[test]
fn the_readonly_list_folds_case_where_case_does_not_matter() {
    let site = Site::new();
    let boundary = site.boundary();
    let upper = site
        .real("work")
        .join(".GIT")
        .join("HOOKS")
        .join("pre-commit");
    assert_eq!(boundary.zone(&upper), Zone::Readable);
}

/// 工作区包住数据根、私密目录（施工 5-14 改：工作目录是 `~` 的就是工作区）：最深的那一片算，数据根照旧谁都不能碰、私密目录照旧
/// 私密、开着的技能文件夹照旧只读；家目录别处能写。工作区落在私密目录里的照工作区（施工 5-13 起就这样）。
#[test]
fn a_workspace_holding_the_data_root_and_private_dirs_keeps_them() {
    let site = Site::new();
    let boundary = Boundary::new(&Places {
        workspace: site.scratch.0.clone(),
        private: vec![site.at("home/.ssh")],
        shelves: vec![site.at("data/home/admin/skills/notes")],
        ..site.places()
    });
    assert_eq!(boundary.zone(&site.real("data/run/token")), Zone::Forbidden);
    assert_eq!(boundary.zone(&site.real("data")), Zone::Forbidden);
    assert_eq!(boundary.zone(&site.real("home/.ssh/id")), Zone::Private);
    assert_eq!(
        boundary.zone(&site.real("data/home/admin/skills/notes/SKILL.md")),
        Zone::Shelf
    );
    assert_eq!(boundary.zone(&site.real("home/notes.txt")), Zone::Writable);
    assert!(
        boundary.blocks_descent(&site.real("data")),
        "往下走目录的工具不进数据根"
    );
    let inside = Boundary::new(&Places {
        workspace: site.at("home/.ssh"),
        private: vec![site.at("home/.ssh")],
        ..site.places()
    });
    assert_eq!(
        inside.zone(&site.real("home/.ssh/id")),
        Zone::Writable,
        "工作区开在私密目录里：照工作区"
    );
}

/// 这个人自己的几处（施工 Y-11 上：家目录里照 XDG 的 pictures 这些）：落在数据根里也照工作区算，还没建的也算；数据根别处照旧
/// 谁都不能碰；往下走目录的工具通得到它。
#[test]
fn the_own_dirs_in_the_data_root_are_writable_even_before_they_exist() {
    let site = Site::new();
    let boundary = Boundary::new(&Places {
        own: vec![site.at("data/home/admin/pictures")],
        ..site.places()
    });
    let data = std::fs::canonicalize(site.at("data")).expect("在");
    assert_eq!(
        boundary.zone(
            &data
                .join("home")
                .join("admin")
                .join("pictures")
                .join("cat.png")
        ),
        Zone::Writable
    );
    assert_eq!(boundary.zone(&site.real("data/run/token")), Zone::Forbidden);
    assert!(!boundary.blocks_descent(&data.join("home")), "通得到它");
    assert!(boundary.blocks_descent(&data.join("run")));
}
