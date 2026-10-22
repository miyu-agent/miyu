use super::*;

fn place() -> Place {
    Place {
        home: PathBuf::from("/home/alice"),
        config: None,
        uid: Some(1000),
        user: Some(r"PC\alice".to_string()),
    }
}

/// 去掉注释、空行的几行。
fn lines(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .collect()
}

#[test]
fn the_user_service_runs_this_miyu_and_keeps_the_data_root() {
    let exe = Path::new("/home/alice/.local/lib/miyu/miyu");
    let made = unit(Manager::Systemd, exe, None, &place()).expect("算得出");
    assert_eq!(
        made.file,
        Path::new("/home/alice/.config/systemd/user/miyu.service")
    );
    assert!(
        lines(&made.text).contains(&"ExecStart=/home/alice/.local/lib/miyu/miyu core"),
        "{}",
        made.text
    );
    assert!(lines(&made.text).contains(&"Restart=on-failure"));
    assert!(!made.text.contains("MIYU_HOME"), "没设的不写");
    let custom = Place {
        config: Some(PathBuf::from("/home/alice/.cfg")),
        ..place()
    };
    let spaced = Path::new("/home/alice/my apps/miyu");
    let made = unit(
        Manager::Systemd,
        spaced,
        Some(Path::new("/data/miyu home")),
        &custom,
    )
    .expect("算得出");
    assert_eq!(
        made.file,
        Path::new("/home/alice/.cfg/systemd/user/miyu.service")
    );
    assert!(
        lines(&made.text).contains(&r#"ExecStart="/home/alice/my apps/miyu" core"#),
        "{}",
        made.text
    );
    assert!(
        lines(&made.text).contains(&r#"Environment="MIYU_HOME=/data/miyu home""#),
        "{}",
        made.text
    );
    assert_eq!(
        enable(Manager::Systemd, &made, &custom),
        [
            vec!["systemctl", "--user", "daemon-reload"],
            vec!["systemctl", "--user", "enable", "--now", "miyu.service"],
        ]
    );
    assert_eq!(
        disable(Manager::Systemd, &custom),
        [vec![
            "systemctl",
            "--user",
            "disable",
            "--now",
            "miyu.service"
        ]]
    );
    assert_eq!(
        running(Manager::Systemd, &custom),
        Some(
            [
                "systemctl",
                "--user",
                "is-active",
                "--quiet",
                "miyu.service"
            ]
            .map(String::from)
            .to_vec()
        )
    );
}

#[test]
fn the_desktop_autostart_runs_this_miyu() {
    let made = unit(
        Manager::Autostart,
        Path::new("/opt/miyu/miyu"),
        Some(Path::new("/data")),
        &place(),
    )
    .expect("算得出");
    assert_eq!(
        made.file,
        Path::new("/home/alice/.config/autostart/miyu.desktop")
    );
    assert!(
        lines(&made.text).contains(&"Exec=env MIYU_HOME=/data /opt/miyu/miyu core"),
        "{}",
        made.text
    );
    assert!(lines(&made.text).contains(&"NoDisplay=true"));
    assert!(
        enable(Manager::Autostart, &made, &place()).is_empty(),
        "下次登录桌面时起"
    );
    assert_eq!(running(Manager::Autostart, &place()), None);
}

#[test]
fn the_launch_agent_runs_this_miyu_and_escapes_the_path() {
    let made = unit(
        Manager::Launchd,
        Path::new("/Users/a&b/miyu"),
        Some(Path::new("/data")),
        &place(),
    )
    .expect("算得出");
    assert_eq!(
        made.file,
        Path::new("/home/alice/Library/LaunchAgents/io.github.miyu-agent.core.plist")
    );
    assert!(
        made.text.contains("<string>/Users/a&amp;b/miyu</string>"),
        "{}",
        made.text
    );
    assert!(
        !made.text.contains("/opt/homebrew/bin/miyu"),
        "样子里的位置换掉了"
    );
    assert!(made.text.contains("<key>MIYU_HOME</key>"), "{}", made.text);
    assert!(
        made.text.contains("<string>/data</string>"),
        "{}",
        made.text
    );
    let file = made.file.to_string_lossy().into_owned();
    assert_eq!(
        enable(Manager::Launchd, &made, &place()),
        [vec![
            "launchctl".to_string(),
            "bootstrap".into(),
            "gui/1000".into(),
            file
        ]]
    );
    assert_eq!(
        disable(Manager::Launchd, &place()),
        [vec![
            "launchctl",
            "bootout",
            "gui/1000/io.github.miyu-agent.core"
        ]]
    );
}

#[test]
fn the_logon_task_runs_this_miyu_for_this_person_only() {
    let exe = Path::new(r"C:\Users\alice\AppData\Local\Programs\Miyu\miyu.exe");
    let windows = Place {
        home: PathBuf::from(r"C:\Users\alice"),
        ..place()
    };
    let made = unit(Manager::Task, exe, None, &windows).expect("算得出");
    assert!(
        made.text
            .contains(r"<Command>C:\Users\alice\AppData\Local\Programs\Miyu\miyu.exe</Command>"),
        "{}",
        made.text
    );
    assert!(
        made.text.contains(r"<UserId>PC\alice</UserId>"),
        "{}",
        made.text
    );
    let file = made.file.to_string_lossy().into_owned();
    assert_eq!(
        enable(Manager::Task, &made, &windows),
        [["schtasks", "/Create", "/TN", "Miyu", "/XML"]
            .map(String::from)
            .to_vec()
            .into_iter()
            .chain([file, "/F".to_string()])
            .collect::<Vec<_>>()]
    );
    assert_eq!(
        disable(Manager::Task, &windows),
        [vec!["schtasks", "/Delete", "/TN", "Miyu", "/F"]]
    );
    assert_eq!(
        unit(Manager::Task, exe, Some(Path::new(r"D:\miyu")), &windows),
        Err(Problem::DataRootOnWindows)
    );
}
