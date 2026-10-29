//! `miyu web`（施工 W-9）：参数照原样交给 `miyu-web open`；没装时说怎么装、退出码 1；装了的照它的退出码。

use super::*;

#[test]
fn the_flags_pass_through_in_order() {
    let all = Web {
        port: Some(8300),
        print: true,
        reset: true,
        logout: true,
        package: Some("onebot".to_string()),
    };
    assert_eq!(
        all.args(),
        [
            "open",
            "--port",
            "8300",
            "--print",
            "--reset",
            "--logout",
            "--package",
            "onebot"
        ]
    );
    assert_eq!(Web::default().args(), ["open"]);
}

#[test]
fn a_missing_web_ui_says_how_to_install() {
    let dir = std::env::temp_dir().join(format!("miyu-cli-web-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("建得了");
    let main = dir.join("miyu");
    std::fs::write(&main, "").expect("写得进");
    let mut err = Vec::new();
    assert_eq!(
        web_on(
            &Web::default(),
            &main,
            Path::new(""),
            "miyu-web",
            Language::Chinese,
            &mut err
        ),
        1
    );
    assert_eq!(
        String::from_utf8(err).expect("UTF-8"),
        "miyu: 未安装网页界面\n提示：用 'miyu pkg install miyu-webui' 安装\n"
    );
    let mut err = Vec::new();
    assert_eq!(
        web_on(
            &Web::default(),
            &main,
            Path::new(""),
            "miyu-web",
            Language::English,
            &mut err
        ),
        1
    );
    assert_eq!(
        String::from_utf8(err).expect("UTF-8"),
        "miyu: no web interface installed\nhint: install it with 'miyu pkg install miyu-webui'\n"
    );
    std::fs::remove_dir_all(&dir).expect("删得掉临时目录");
}

#[cfg(unix)]
#[test]
fn an_installed_web_ui_gets_the_args_and_decides_the_exit_code() {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::temp_dir().join(format!("miyu-cli-web-ok-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("建得了");
    let main = dir.join("miyu");
    std::fs::write(&main, "").expect("写得进");
    let seen = dir.join("seen");
    let program = dir.join("miyu-web");
    let told = dir.join("told");
    std::fs::write(
        &program,
        format!(
            "#!/bin/sh\necho \"$@\" > {}\necho \"$MIYU_PROGRAM\" > {}\nexit 7\n",
            seen.display(),
            told.display()
        ),
    )
    .expect("写得进");
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).expect("改得了");
    let args = Web {
        reset: true,
        ..Web::default()
    };
    let mut err = Vec::new();
    assert_eq!(
        web_on(
            &args,
            &main,
            Path::new(""),
            "miyu-web",
            Language::Chinese,
            &mut err
        ),
        7,
        "照它的退出码"
    );
    assert_eq!(
        std::fs::read_to_string(&seen).expect("跑了").trim(),
        "open --reset"
    );
    assert_eq!(
        std::fs::read_to_string(&told).expect("跑了").trim(),
        std::fs::canonicalize(&main)
            .expect("在")
            .display()
            .to_string(),
        "主程序的真实位置交给它（施工 G-15）：装进数据根的网页软件旁边没有主程序"
    );
    std::fs::remove_dir_all(&dir).expect("删得掉临时目录");
}
