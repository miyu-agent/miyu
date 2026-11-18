//! 挪走、扔进回收站照删认（施工 RO-4）：整个数据根、装着它的照删数据根问；数据根里面的单个文件不问，`rm` 和回收站一个口径。

use super::super::*;
use super::{PLACES, first, kind};

/// 文档会话实测 `gio trash ~/.miyu` 不问、整个数据根进了回收站；同类的几种一起认，照删数据根问、归删除那一类。
#[test]
fn trashing_or_moving_the_whole_data_root_asks_as_deleting_it() {
    for command in [
        "gio trash ~/.miyu",
        "gio trash -f /home/me/.miyu/",
        "gio trash file:///home/me/.miyu",
        "gio remove -f ~/.miyu",
        "gio move ~/.miyu /tmp/old",
        "gio rename ~/.miyu old-miyu",
        "trash-put ~/.miyu",
        "trash ~/.miyu",
        "gvfs-trash ~/.miyu",
        "kioclient5 move ~/.miyu trash:/",
        "kioclient6 --noninteractive remove ~/.miyu",
        "mv ~/.miyu ~/.miyu.bak",
        "mv -f -- ~/.miyu /tmp/x",
        "mv -t /tmp ~/.miyu",
        "mv -ft /tmp ~/.miyu",
        "mv --target-directory=/tmp ~/.miyu",
        "sudo mv /home/me/.miyu /srv/old",
        "rsync -a --remove-source-files ~/.miyu/ /backup/",
        "rsync -a --exclude '*.tmp' --remove-source-files ~/.miyu /backup/",
        "D=~/.miyu; trash-put \"$D\"",
        "cd ~ && mv .miyu .miyu-old",
    ] {
        assert_eq!(kind(command), Some("delete_data_root"), "{command}");
        let found = first(command, &PLACES).expect("认得出");
        assert!(!found.refused(), "{command}：问人，不是拦");
        assert_eq!(found.group(), Some(Group::Delete), "{command}");
    }
    // 数据根不在家目录里：挪走装着它的那一层也算。
    let places = Places {
        data_root: "/srv/miyu/data",
        ..PLACES
    };
    assert_eq!(
        first("gio trash /srv/miyu", &places).map(Kind::code),
        Some("delete_data_root")
    );
}

/// 挪走、扔进回收站的口径同 `rm`：家目录、根、系统目录拒，工作目录问。
#[test]
fn trashing_or_moving_the_big_places_is_judged_like_deleting_them() {
    for (command, expected) in [
        ("gio trash ~", "delete_home"),
        ("trash-put /home/me", "delete_home"),
        ("mv /usr /usr.old", "delete_system"),
        ("gio trash .", "delete_working_dir"),
        ("mv ../proj ../proj2", "delete_working_dir"),
    ] {
        assert_eq!(kind(command), Some(expected), "{command}");
    }
}

/// 不报的：数据根里面的单个文件、目录（`rm` 也不再问），挪进数据根的，不挪走的 `rsync`，别的子命令，来源说不清的。
#[test]
fn files_inside_the_data_root_and_ordinary_moves_do_not_ask() {
    for command in [
        "rm ~/.miyu/sessions/s1/log.jsonl",
        "rm -rf ~/.miyu/sessions/s1",
        "shred -u ~/.miyu/token",
        "find ~/.miyu/cache -delete",
        "gio trash ~/.miyu/etc/config.toml",
        "trash-put ~/.miyu/logs",
        "mv ~/.miyu/a.txt ~/b.txt",
        "mv notes.md ~/.miyu",
        "mv -t ~/.miyu a.txt",
        "rsync -a ~/.miyu/ /backup/",
        "rsync -a --remove-source-files build/ ~/.miyu",
        "gio info ~/.miyu",
        "gio copy ~/.miyu /backup",
        "gio trash --empty",
        "kioclient5 copy ~/.miyu /backup",
        "trash-list",
        "mv \"$f\" /tmp/",
        "trash-put build dist",
    ] {
        assert_eq!(kind(command), None, "{command}");
    }
}
