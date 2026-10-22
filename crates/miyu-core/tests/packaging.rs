//! 让核心一直开着的几份托管写法（施工 Z-1 上，`docs/designs/12-进程形态与分发.md` 第二节）：`packaging/` 里每一份读得成，
//! 跑的是 `miyu core`（网页软件那一份是 `miyu web serve`）；服务器的样例用专门的用户 `miyu`、数据根 `/var/lib/miyu`，不写拦了
//! bubblewrap 的 `RestrictNamespaces=`。

use std::path::{Path, PathBuf};

/// 仓库里 `packaging/` 下的一份。
fn packaged(relative: &str) -> String {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packaging")
        .join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} 读得出：{error}", path.display()))
}

/// systemd 单元里不是注释的那几行：`键=值`。
fn unit_lines(text: &str) -> Vec<(String, String)> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with('['))
        .map(|line| {
            let (key, value) = line
                .split_once('=')
                .unwrap_or_else(|| panic!("不是 键=值：{line}"));
            (key.to_string(), value.to_string())
        })
        .collect()
}

/// 单元里 `key` 的那几个值。
fn values<'a>(lines: &'a [(String, String)], key: &str) -> Vec<&'a str> {
    lines
        .iter()
        .filter(|(name, _)| name == key)
        .map(|(_, value)| value.as_str())
        .collect()
}

#[test]
fn the_user_service_runs_the_core_and_restarts_it_on_failure() {
    let lines = unit_lines(&packaged("systemd/user/miyu.service"));
    assert!(values(&lines, "ExecStart")[0].ends_with("/miyu core"));
    assert_eq!(values(&lines, "Restart"), ["on-failure"]);
    assert_eq!(values(&lines, "WantedBy"), ["default.target"]);
}

#[test]
fn the_server_samples_run_as_their_own_user_and_keep_the_sandbox_working() {
    for (file, command) in [
        ("systemd/system/miyu.service", "/miyu core"),
        ("systemd/system/miyu-web.service", "/miyu-web serve"),
    ] {
        let lines = unit_lines(&packaged(file));
        assert!(values(&lines, "ExecStart")[0].ends_with(command), "{file}");
        assert_eq!(values(&lines, "User"), ["miyu"], "{file}");
        assert!(
            values(&lines, "Environment").contains(&"MIYU_HOME=/var/lib/miyu"),
            "{file}"
        );
        assert!(
            values(&lines, "RestrictNamespaces").is_empty(),
            "{file}：会拦 bubblewrap"
        );
        assert_eq!(values(&lines, "WantedBy"), ["multi-user.target"], "{file}");
    }
    let core = unit_lines(&packaged("systemd/system/miyu.service"));
    assert_eq!(values(&core, "StateDirectory"), ["miyu"]);
    let web = unit_lines(&packaged("systemd/system/miyu-web.service"));
    assert_eq!(values(&web, "Requires"), ["miyu.service"]);
}

#[test]
fn the_desktop_autostart_runs_the_core_out_of_sight() {
    let lines = unit_lines(&packaged("xdg/miyu.desktop"));
    assert_eq!(values(&lines, "Type"), ["Application"]);
    assert_eq!(values(&lines, "Exec"), ["miyu core"]);
    assert_eq!(values(&lines, "NoDisplay"), ["true"]);
}

#[test]
fn the_launch_agent_and_the_logon_task_run_the_core() {
    let plist = packaged("macos/miyu.plist");
    let document = roxmltree::Document::parse_with_options(
        &plist,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            ..roxmltree::ParsingOptions::default()
        },
    )
    .expect("plist 是合写法的 XML");
    let strings: Vec<&str> = document
        .descendants()
        .filter(|node| node.has_tag_name("string"))
        .filter_map(|node| node.text())
        .collect();
    assert!(strings.contains(&"core"), "{strings:?}");
    assert!(
        document
            .descendants()
            .any(|node| node.has_tag_name("key") && node.text() == Some("RunAtLoad"))
    );
    let task = packaged("windows/miyu.xml");
    let document = roxmltree::Document::parse(&task).expect("任务计划是合写法的 XML");
    let arguments = document
        .descendants()
        .find(|node| node.tag_name().name() == "Arguments")
        .and_then(|node| node.text());
    assert_eq!(arguments, Some("core"));
    assert!(
        document
            .descendants()
            .any(|node| node.tag_name().name() == "LogonTrigger"),
        "登录时启动"
    );
}

#[test]
fn the_other_init_samples_run_the_core_as_its_own_user() {
    for file in ["init/openrc/miyu", "init/runit/miyu/run", "init/dinit/miyu"] {
        let text = packaged(file);
        assert!(
            text.contains("miyu core") || text.contains("command_args=\"core\""),
            "{file}"
        );
        assert!(text.contains("MIYU_HOME=/var/lib/miyu"), "{file}");
    }
    assert!(packaged("README.md").contains("RestrictNamespaces"));
    // 网页软件默认空闲 600 秒就退、退出码 0（W-349 起写 0 是不退）：服务器上要写 0，不然退了服务管理器不再起它。
    assert!(packaged("README.md").contains("idle_seconds = 0"));
}
