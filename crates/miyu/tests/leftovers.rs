//! 核心起来时收拾上一次崩了留下的（发布前第二轮审查，施工 AE-7、AS-11）：升级装上的包时两次改名之间被杀，`<编号>/` 没了、
//! 原来那一份还在 `.<编号>.old`，起来时放回去；写配置、密钥时被杀留下的临时文件 `.<文件名>.<进程号>-<计数>.tmp`，
//! 放了一阵的起来时删掉（里面可能是密钥的副本）。认不出的名字一个都不碰。

use std::fs;
use std::time::{Duration, SystemTime};

use miyu_ipc::connect_or_start;

use crate::support::{Home, hello, within};

/// 起一次核心，握个手，等它空闲了自己走：起来的那一段做完了。
async fn start_once(home: &Home) {
    let (connection, token) = within("拉起", connect_or_start(&home.root, || home.core()))
        .await
        .expect("拉得起");
    let reply = hello(connection, &token).await;
    assert!(reply.get("result").is_some(), "{reply}");
    home.until_stopped().await;
}

#[tokio::test]
async fn an_upgrade_cut_off_between_the_renames_is_put_back_when_the_core_starts() {
    let home = Home::new();
    let packages = home.root.packages();
    // 原来那一份已经挪到 `.<编号>.old`，暂存的 `.<编号>.new` 还没换进去：核心就在这时被杀了。
    fs::create_dir_all(packages.join(".pudding.old")).expect("建得了");
    fs::write(packages.join(".pudding.old").join("package.toml"), "old").expect("写得进");
    fs::create_dir_all(packages.join(".pudding.new")).expect("建得了");
    fs::write(packages.join(".pudding.new").join("package.toml"), "new").expect("写得进");
    // 换好了、备份没删掉的：本体在，备份是多余的。
    fs::create_dir_all(packages.join("cake")).expect("建得了");
    fs::write(packages.join("cake").join("package.toml"), "cake").expect("写得进");
    fs::create_dir_all(packages.join(".cake.old")).expect("建得了");
    // 人放的、认不出的：不碰。
    fs::create_dir_all(packages.join(".pudding.bak")).expect("建得了");
    fs::create_dir_all(packages.join(".Not An Id.old")).expect("建得了");
    start_once(&home).await;
    assert_eq!(
        fs::read_to_string(packages.join("pudding").join("package.toml")).ok(),
        Some("old".to_string()),
        "原来那一份放回去了"
    );
    for gone in [".pudding.old", ".pudding.new", ".cake.old"] {
        assert!(
            !packages.join(gone).exists(),
            "{gone} 是上一次剩下的，该清掉"
        );
    }
    assert!(packages.join("cake").join("package.toml").is_file());
    for kept in [".pudding.bak", ".Not An Id.old"] {
        assert!(packages.join(kept).exists(), "{kept} 认不出，不该碰");
    }
}

#[tokio::test]
async fn temporary_copies_left_by_a_crash_are_removed_when_the_core_starts() {
    let home = Home::new();
    let root = home.root.path();
    let day_ago = SystemTime::now() - Duration::from_secs(24 * 3600);
    // 配置、密钥的临时文件建在 `run/` 里（施工 R3-1）。
    fs::create_dir_all(root.join("run")).expect("建得了");
    let put = |name: &str, old: bool| {
        let path = root.join(name);
        fs::create_dir_all(path.parent().expect("有上一级")).expect("建得了");
        fs::write(&path, "secret = \"sk-test\"").expect("写得进");
        if old {
            fs::File::options()
                .write(true)
                .open(&path)
                .and_then(|file| file.set_modified(day_ago))
                .expect("改得了时刻");
        }
        path
    };
    let stale = [
        put("run/.secrets.toml.4242-1.tmp", true),
        put("run/.trust.toml.4242-2.tmp", true),
        // 核心自己的短命暂存起来就清（设计 55 第三节第 5 条）。
        put("tmp/unpack/123-456/package.toml", false),
    ];
    let kept = [
        // 刚写的：说不定是别的程序正在写，不碰。
        put("run/.secrets.toml.4242-9.tmp", false),
        // 认不出的名字：人放的，不碰。
        put("run/.config.toml.tmp", true),
        put("run/notes.tmp", true),
        put("run/.config.toml.abc-1.tmp", true),
        // 包的短命暂存在拉起它之前清，不在这时。
        put("tmp/web/page.html", false),
    ];
    start_once(&home).await;
    for path in stale {
        assert!(
            !path.exists(),
            "{} 是崩了留下的临时文件，该删掉",
            path.display()
        );
    }
    for path in kept {
        assert!(path.exists(), "{} 不该碰", path.display());
    }
}

/// 设计 55 第五节第 2 条那张表核心起来时就建好（施工 R3-1 补、FH-3）：Windows 的沙盒只禁命令起来那一刻已有的，命令跑着时
/// 第一次建的（第一次存密钥、删会话挪进回收处、第一次存 blob、第一次记记忆）整棵读得到。顶层只有布局里的几样。
#[tokio::test]
async fn the_sensitive_places_are_made_when_the_core_starts() {
    let home = Home::new();
    start_once(&home).await;
    let made = [
        "etc",
        "etc/private",
        "etc/removed",
        "opt",
        "root",
        "root/sessions",
        "root/memory",
        "root/kb",
        "root/blobs",
        "root/trash",
        "root/state",
        "root/workspace",
        "var/lib",
        "var/lib/packages",
        "var/lib/mcp",
        "var/cache",
        "var/cache/index",
        "var/log",
        "var/tmp",
        "tmp",
        "run",
    ];
    for dir in made {
        assert!(home.root.path().join(dir).is_dir(), "{dir}/ 起来时就该在");
    }
    let mut top: Vec<String> = fs::read_dir(home.root.path())
        .expect("读得了")
        .map(|entry| {
            entry
                .expect("读得了")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    top.sort();
    assert_eq!(
        top,
        [".miyu-layout", "etc", "opt", "root", "run", "tmp", "var"]
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(home.root.path().join("etc/private"))
            .expect("在")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o700, "凭据的目录只给本人");
    }
}
