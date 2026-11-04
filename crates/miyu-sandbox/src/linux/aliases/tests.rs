use super::*;

/// 挂载表的一行（前两格、选项和后面的照真的样子随便填）。
fn line(dev: &str, root: &str, point: &str) -> String {
    format!("36 25 {dev} {root} {point} rw,relatime shared:1 - ext4 /dev/sda1 rw\n")
}

fn paths(pairs: &[(PathBuf, PathBuf)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(a, b)| (a.display().to_string(), b.display().to_string()))
        .collect()
}

#[test]
fn the_table_is_read_with_escapes_and_bad_lines_skipped() {
    let table = format!(
        "{}{}too short\n",
        line("8:1", "/", "/"),
        line("8:1", "/data/my\\040home", "/home\\134x")
    );
    assert_eq!(
        parse(table.as_bytes()),
        [
            Mount {
                dev: b"8:1".to_vec(),
                root: PathBuf::from("/"),
                point: PathBuf::from("/"),
            },
            Mount {
                dev: b"8:1".to_vec(),
                root: PathBuf::from("/data/my home"),
                point: PathBuf::from("/home\\x"),
            },
        ]
    );
}

#[test]
fn a_bind_mounted_home_is_reached_from_the_whole_disk_too() {
    // `/home` 是 `/data/home` bind 过来的：数据根在 `/data/home/me/.miyu` 也看得到。
    let table = format!(
        "{}{}{}",
        line("8:1", "/", "/"),
        line("8:1", "/data/home", "/home"),
        line("8:2", "/", "/mnt/other")
    );
    let found = candidates(Path::new("/home/me/.miyu"), &parse(table.as_bytes()));
    assert_eq!(
        paths(&found),
        [(
            "/data/home/me/.miyu".to_string(),
            "/home/me/.miyu".to_string()
        )]
    );
}

#[test]
fn the_btrfs_top_level_mounted_elsewhere_is_an_alias() {
    // 根是子卷 `/@`、家是 `/@home`，顶层（`/`）另挂在 `/mnt/top`：家目录在 `/mnt/top/@home` 也看得到。
    let table = format!(
        "{}{}{}",
        line("0:30", "/@", "/"),
        line("0:30", "/@home", "/home"),
        line("0:30", "/", "/mnt/top")
    );
    let found = candidates(Path::new("/home/me/.miyu"), &parse(table.as_bytes()));
    assert_eq!(
        paths(&found),
        [(
            "/mnt/top/@home/me/.miyu".to_string(),
            "/home/me/.miyu".to_string()
        )]
    );
}

#[test]
fn a_piece_of_the_hidden_mounted_elsewhere_is_hidden_as_a_whole() {
    // 数据根里的 `run` 挂到了 `/srv/run`：那个挂载点整个对着 `.miyu/run`。
    let table = format!(
        "{}{}",
        line("8:1", "/", "/"),
        line("8:1", "/home/me/.miyu/run", "/srv/run")
    );
    let found = candidates(Path::new("/home/me/.miyu"), &parse(table.as_bytes()));
    assert_eq!(
        paths(&found),
        [("/srv/run".to_string(), "/home/me/.miyu/run".to_string())]
    );
}

#[test]
fn the_innermost_and_last_stacked_mount_is_where_the_path_lives() {
    // `/home` 先挂了 8:1 的，又叠了一层 8:2 的：路径在后挂的 8:2 里，8:1 的那一片不算它的别名。
    let table = format!(
        "{}{}{}{}",
        line("8:1", "/", "/"),
        line("8:1", "/old", "/home"),
        line("8:2", "/", "/home"),
        line("8:2", "/", "/mnt/two")
    );
    let found = candidates(Path::new("/home/me/.miyu"), &parse(table.as_bytes()));
    assert_eq!(
        paths(&found),
        [(
            "/mnt/two/me/.miyu".to_string(),
            "/home/me/.miyu".to_string()
        )]
    );
}

#[test]
fn without_another_mount_of_the_same_filesystem_there_is_no_alias() {
    let table = format!("{}{}", line("8:1", "/", "/"), line("8:2", "/", "/home"));
    assert!(candidates(Path::new("/home/me/.miyu"), &parse(table.as_bytes())).is_empty());
    // 真的挂载表、真的路径：头一个是它自己，别的（有的话）都和它是同一个文件。
    let dir = fs::canonicalize(std::env::temp_dir()).expect("在");
    let found = with_aliases(std::slice::from_ref(&dir));
    assert_eq!(found.first(), Some(&dir));
    assert!(
        found.iter().all(|alias| same_file(alias, &dir)),
        "{found:?}"
    );
}

#[test]
fn only_the_same_file_counts_as_an_alias() {
    // 挂载表说 `real` 也挂在了 `elsewhere`：那里真是同一个目录的才藏，那里另有一个不相干的目录的不藏。
    let base = fs::canonicalize(std::env::temp_dir())
        .expect("在")
        .join(format!("miyu-sandbox-aliases-{}", std::process::id()));
    let real = base.join("real");
    let elsewhere = base.join("elsewhere");
    for dir in [real.join(".miyu"), elsewhere.join(".miyu")] {
        fs::create_dir_all(dir).expect("建得了");
    }
    let hidden = real.join(".miyu");
    let table = format!(
        "{}{}",
        line("8:1", "/", "/"),
        line(
            "8:1",
            &real.display().to_string(),
            &elsewhere.display().to_string()
        )
    );
    let mounts = parse(table.as_bytes());
    assert_eq!(
        candidates(&hidden, &mounts),
        [(elsewhere.join(".miyu"), hidden.clone())],
        "挂载表说是别名"
    );
    assert_eq!(
        with_aliases_in(std::slice::from_ref(&hidden), &mounts),
        std::slice::from_ref(&hidden),
        "不是同一个目录，不藏"
    );
    std::os::unix::fs::symlink(&hidden, base.join("same")).expect("建得了链接");
    let table = format!(
        "{}{}",
        line("8:1", "/", "/"),
        line(
            "8:1",
            &hidden.display().to_string(),
            &base.join("same").display().to_string()
        )
    );
    assert_eq!(
        with_aliases_in(std::slice::from_ref(&hidden), &parse(table.as_bytes())),
        [hidden, base.join("same")],
        "是同一个目录，一起藏"
    );
    fs::remove_dir_all(&base).expect("删得掉");
}
