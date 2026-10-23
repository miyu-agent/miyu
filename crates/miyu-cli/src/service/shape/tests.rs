use std::path::PathBuf;

use serde_json::json;

use super::shaped;
use crate::service::{Manager, Problem, Said};

fn file() -> PathBuf {
    PathBuf::from("/home/alice/.config/systemd/user/miyu.service")
}

#[test]
fn every_success_has_the_same_four_fields() {
    let path = "/home/alice/.config/systemd/user/miyu.service";
    assert_eq!(
        shaped(&Said::Enabled(Manager::Systemd, file())),
        json!({"installed": true, "on": true, "file": path, "manager": "systemd"})
    );
    assert_eq!(
        shaped(&Said::Enabled(Manager::Autostart, file())),
        json!({"installed": true, "on": null, "file": path, "manager": "autostart"}),
        "自启动问不了开没开"
    );
    assert_eq!(
        shaped(&Said::Disabled(Manager::Launchd, file())),
        json!({"installed": false, "on": false, "file": path, "manager": "launchd"})
    );
    assert_eq!(
        shaped(&Said::NotEnabled(Manager::Task, file())),
        json!({"installed": false, "on": false, "file": path, "manager": "task"})
    );
    let status = |installed, on| Said::Status {
        manager: Manager::Systemd,
        file: file(),
        installed,
        on,
    };
    assert_eq!(
        shaped(&status(true, Some(false))),
        json!({"installed": true, "on": false, "file": path, "manager": "systemd"})
    );
    assert_eq!(
        shaped(&status(false, None)),
        json!({"installed": false, "on": false, "file": path, "manager": "systemd"}),
        "没装的就是没开"
    );
    assert_eq!(
        shaped(&Said::Status {
            manager: Manager::Autostart,
            file: file(),
            installed: true,
            on: None
        })["on"],
        json!(null)
    );
}

#[test]
fn failures_say_a_code_the_step_and_the_system_s_words() {
    assert_eq!(
        shaped(&Said::Failed(
            "systemctl --user enable --now miyu.service".into(),
            "boom".into()
        )),
        json!({"error": "failed", "step": "systemctl --user enable --now miyu.service", "said": "boom"})
    );
    assert_eq!(
        shaped(&Said::Cannot(Problem::DataRootOnWindows)),
        json!({"error": "data_root_on_windows"})
    );
    assert_eq!(shaped(&Said::NoHome), json!({"error": "no_home"}));
}
