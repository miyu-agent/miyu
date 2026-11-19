//! 边界表（施工 4-3 上）：几片重叠时照先后，先对上的算。

use std::path::PathBuf;

use crate::support::Site;
use miyu_fs::{Boundary, Places, Zone, within};

#[test]
fn the_workspace_is_writable_all_the_way_down() {
    let site = Site::new();
    let boundary = site.boundary();
    assert_eq!(boundary.zone(&site.real("work/src/a.rs")), Zone::Writable);
    assert_eq!(boundary.zone(&site.real("work")), Zone::Writable);
    assert_eq!(boundary.zone(&site.real("work/.git/HEAD")), Zone::Writable);
    // 只禁写的名单撤了（施工 LV-5）：`.git/hooks` 这些和工作区里别的一样。
    assert_eq!(
        boundary.zone(&site.real("work/.git/hooks/pre-commit")),
        Zone::Writable
    );
    // 还不存在的也照位置算：换成真实的位置由 resolve 做，这里直接接上。
    assert_eq!(
        boundary.zone(&site.real("work").join("new.rs")),
        Zone::Writable
    );
}

#[test]
fn the_data_root_is_its_own_zone_even_inside_the_temp_dir() {
    let site = Site::new();
    let boundary = site.boundary();
    assert_eq!(boundary.zone(&site.real("data/run/token")), Zone::DataRoot);
    assert_eq!(boundary.zone(&site.real("data")), Zone::DataRoot);
    // 临时目录把数据根包在里面（测试、MIYU_HOME 指到那里）：数据根照样算数据根那一片。
    let wide = Boundary::new(&Places {
        temp: site.scratch.0.clone(),
        ..site.places()
    });
    assert_eq!(wide.zone(&site.real("data/run/token")), Zone::DataRoot);
    assert_eq!(wide.zone(&site.real("tmp/t.txt")), Zone::Writable);
}

#[test]
fn a_workspace_moved_into_the_data_root_is_still_a_workspace() {
    let site = Site::new();
    let boundary = Boundary::new(&Places {
        workspace: site.at("data/root/workspace"),
        ..site.places()
    });
    assert_eq!(
        boundary.zone(&site.real("data/root/workspace/w.txt")),
        Zone::Writable
    );
    assert_eq!(boundary.zone(&site.real("data/run/token")), Zone::DataRoot);
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

/// 加进来的目录落进了数据根（例如报来以后被换成了链接）：数据根照样算数据根那一片。
#[test]
fn the_data_root_wins_over_an_added_dir() {
    let site = Site::new();
    let boundary = Boundary::new(&Places {
        dirs: vec![site.at("data"), site.at("data/home")],
        ..site.places()
    });
    assert_eq!(boundary.zone(&site.real("data/run/token")), Zone::DataRoot);
    assert_eq!(
        boundary.zone(&site.real("data/root/workspace/w.txt")),
        Zone::DataRoot
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
        shelves: vec![site.at("data/root/skills/notes"), site.at("nowhere")],
        ..site.places()
    });
    let skill = site.real("data/root/skills/notes/SKILL.md");
    assert_eq!(boundary.zone(&skill), Zone::Shelf);
    assert_eq!(
        boundary.zone(&site.real("data/root/skills/notes")),
        Zone::Shelf
    );
    // 旁边的照旧算数据根那一片。
    assert_eq!(
        boundary.zone(&site.real("data/root/skills")),
        Zone::DataRoot
    );
    assert_eq!(boundary.zone(&site.real("data/run/token")), Zone::DataRoot);
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
        Zone::DataRoot
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

/// 别的程序的钥匙、登录不再单是一片（施工 PV-RM）：`~/.ssh` 和家目录别处一样在边界以外；数据根照旧算数据根那一片。
#[test]
fn a_key_folder_is_outside_like_the_rest_of_the_home() {
    let site = Site::new();
    let boundary = site.boundary();
    assert_eq!(boundary.zone(&site.real("home/.ssh/id")), Zone::Outside);
    assert_eq!(boundary.zone(&site.real("home/.ssh")), Zone::Outside);
    assert_eq!(boundary.zone(&site.real("home/notes.txt")), Zone::Outside);
    assert_eq!(boundary.zone(&site.real("data/run/token")), Zone::DataRoot);
}

/// 工作区包住数据根（施工 5-14 改：工作目录是 `~` 的就是工作区）：最深的那一片算，数据根照旧算数据根那一片、开着的技能文件夹照旧
/// 只读；家目录别处（钥匙在内）能写。
#[test]
fn a_workspace_holding_the_data_root_keeps_it() {
    let site = Site::new();
    let boundary = Boundary::new(&Places {
        workspace: site.scratch.0.clone(),
        shelves: vec![site.at("data/root/skills/notes")],
        ..site.places()
    });
    assert_eq!(boundary.zone(&site.real("data/run/token")), Zone::DataRoot);
    assert_eq!(boundary.zone(&site.real("data")), Zone::DataRoot);
    assert_eq!(
        boundary.zone(&site.real("data/root/skills/notes/SKILL.md")),
        Zone::Shelf
    );
    assert_eq!(boundary.zone(&site.real("home/.ssh/id")), Zone::Writable);
    assert_eq!(boundary.zone(&site.real("home/notes.txt")), Zone::Writable);
    assert!(
        boundary.blocks_descent(&site.real("data")),
        "往下走目录的工具不进数据根"
    );
}

/// 这个人自己的几处（施工 Y-11 上：家目录里照 XDG 的 pictures 这些）：落在数据根里也照工作区算，还没建的也算；数据根别处照旧算
/// 数据根那一片；往下走目录的工具通得到它。
#[test]
fn the_own_dirs_in_the_data_root_are_writable_even_before_they_exist() {
    let site = Site::new();
    let boundary = Boundary::new(&Places {
        own: vec![site.at("data/root/pictures")],
        ..site.places()
    });
    let data = std::fs::canonicalize(site.at("data")).expect("在");
    assert_eq!(
        boundary.zone(&data.join("root/pictures").join("cat.png")),
        Zone::Writable
    );
    assert_eq!(boundary.zone(&site.real("data/run/token")), Zone::DataRoot);
    assert!(!boundary.blocks_descent(&data), "通得到它");
    assert!(boundary.blocks_descent(&data.join("run")));
}

/// 数据根的上级目录是链接（macOS 的临时目录在 `/var` 下，`/var` 是到 `/private/var` 的链接；Linux 上 `~/.config` 这类被链到
/// 别处）：记下的是真实位置，目标照 `resolve` 换成真实位置以后落在数据根那一片。数据根还没建的也一样：照最近的、已经存在的
/// 上级换，不因为换不成就不算这一片（LiteMiyu 上撞见过同一类问题：记的是原样的路径，目标是真实位置，两边对不上就拦不住）。
#[cfg(unix)]
#[test]
fn the_data_root_under_a_linked_parent_is_known_whether_it_is_made_or_not() {
    let site = Site::new();
    std::fs::create_dir_all(site.at("disk/made/etc")).expect("建得了");
    std::os::unix::fs::symlink(site.at("disk"), site.at("link")).expect("建得了链接");
    let target = |path: &str| miyu_fs::resolve(&site.scratch.0, None, path).expect("换得成");
    // 建好的：照链接说的、照真实位置说的都算。
    let made = Boundary::new(&Places {
        data_root: site.at("link/made"),
        ..site.places()
    });
    assert_eq!(
        made.zone(&target("link/made/etc/secrets.toml")),
        Zone::DataRoot
    );
    assert_eq!(
        made.zone(&target("disk/made/etc/secrets.toml")),
        Zone::DataRoot
    );
    // 还没建的：目标换成的真实位置在链接指到的那一边，记的也得是那一边。
    let later = Boundary::new(&Places {
        data_root: site.at("link/later"),
        ..site.places()
    });
    assert!(!site.at("disk/later").exists());
    assert_eq!(
        later.zone(&target("link/later/etc/secrets.toml")),
        Zone::DataRoot
    );
    assert_eq!(
        later.zone(&target("disk/later/etc/secrets.toml")),
        Zone::DataRoot
    );
    assert_eq!(later.zone(&target("disk/other.txt")), Zone::Outside);
}
