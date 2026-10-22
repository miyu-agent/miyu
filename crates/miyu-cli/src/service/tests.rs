//! 装、卸、看怎么走（施工 Z-1 下）：经替身机器走，不碰真的服务管理器。

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use super::machine::{Machine, Ran};
use super::plan::{Manager, Place, Problem};
use super::{Said, ServiceAction, flow};

/// 替身：记下写了什么、跑了什么；跑的命令照 `fails` 里的开头失败。
struct Fake {
    manager: Manager,
    home: Option<PathBuf>,
    data_root: Option<PathBuf>,
    files: RefCell<BTreeMap<PathBuf, String>>,
    ran: RefCell<Vec<String>>,
    fails: Vec<&'static str>,
}

impl Fake {
    fn new(manager: Manager) -> Fake {
        Fake {
            manager,
            home: Some(PathBuf::from("/home/alice")),
            data_root: None,
            files: RefCell::new(BTreeMap::new()),
            ran: RefCell::new(Vec::new()),
            fails: Vec::new(),
        }
    }

    fn ran(&self) -> Vec<String> {
        self.ran.borrow().clone()
    }
}

impl Machine for Fake {
    fn manager(&self) -> Manager {
        self.manager
    }

    fn place(&self) -> Option<Place> {
        Some(Place {
            home: self.home.clone()?,
            config: None,
            uid: Some(501),
            user: Some(r"PC\alice".to_string()),
        })
    }

    fn exe(&self) -> io::Result<PathBuf> {
        Ok(PathBuf::from("/opt/miyu/miyu"))
    }

    fn data_root(&self) -> Option<PathBuf> {
        self.data_root.clone()
    }

    fn write(&self, file: &Path, text: &str) -> io::Result<()> {
        self.files
            .borrow_mut()
            .insert(file.to_path_buf(), text.to_string());
        Ok(())
    }

    fn remove(&self, file: &Path) -> io::Result<()> {
        match self.files.borrow_mut().remove(file) {
            Some(_) => Ok(()),
            None => Err(io::Error::from(io::ErrorKind::NotFound)),
        }
    }

    fn exists(&self, file: &Path) -> bool {
        self.files.borrow().contains_key(file)
    }

    fn run(&self, command: &[String]) -> Ran {
        let line = command.join(" ");
        self.ran.borrow_mut().push(line.clone());
        let failed = self.fails.iter().any(|start| line.starts_with(start));
        Ran {
            ok: !failed,
            said: if failed {
                "boom".to_string()
            } else {
                String::new()
            },
        }
    }
}

const UNIT: &str = "/home/alice/.config/systemd/user/miyu.service";

#[test]
fn enabling_writes_the_unit_then_hands_it_to_systemd() {
    let fake = Fake::new(Manager::Systemd);
    let said = flow::run(&fake, ServiceAction::Enable);
    assert_eq!(said, Said::Enabled(Manager::Systemd, PathBuf::from(UNIT)));
    assert!(fake.files.borrow()[Path::new(UNIT)].contains("ExecStart=/opt/miyu/miyu core"));
    assert_eq!(
        fake.ran(),
        [
            "systemctl --user daemon-reload",
            "systemctl --user enable --now miyu.service"
        ]
    );
    let status = flow::run(&fake, ServiceAction::Status);
    assert_eq!(
        status,
        Said::Status {
            file: PathBuf::from(UNIT),
            installed: true,
            on: Some(true)
        }
    );
}

#[test]
fn a_failing_step_says_which_and_why() {
    let fake = Fake {
        fails: vec!["systemctl --user enable"],
        ..Fake::new(Manager::Systemd)
    };
    let said = flow::run(&fake, ServiceAction::Enable);
    assert_eq!(
        said,
        Said::Failed(
            "systemctl --user enable --now miyu.service".to_string(),
            "boom".to_string()
        )
    );
    assert_eq!(said.code(), 1);
}

#[test]
fn disabling_stops_it_then_deletes_the_file_and_twice_says_not_installed() {
    let fake = Fake::new(Manager::Systemd);
    assert_eq!(
        flow::run(&fake, ServiceAction::Disable),
        Said::NotEnabled,
        "没装的不跑命令"
    );
    assert!(fake.ran().is_empty());
    flow::run(&fake, ServiceAction::Enable);
    fake.ran.borrow_mut().clear();
    assert_eq!(
        flow::run(&fake, ServiceAction::Disable),
        Said::Disabled(PathBuf::from(UNIT))
    );
    assert_eq!(fake.ran(), ["systemctl --user disable --now miyu.service"]);
    assert!(fake.files.borrow().is_empty(), "删了");
    assert_eq!(
        flow::run(&fake, ServiceAction::Status),
        Said::Status {
            file: PathBuf::from(UNIT),
            installed: false,
            on: None
        }
    );
}

#[test]
fn launchd_takes_out_an_old_one_first_and_a_missing_one_is_fine() {
    let fake = Fake {
        fails: vec!["launchctl bootout"],
        ..Fake::new(Manager::Launchd)
    };
    let said = flow::run(&fake, ServiceAction::Enable);
    assert!(
        matches!(said, Said::Enabled(Manager::Launchd, _)),
        "{said:?}"
    );
    let ran = fake.ran();
    assert_eq!(
        ran[0],
        "launchctl bootout gui/501/io.github.miyu-agent.core"
    );
    assert!(
        ran[1].starts_with("launchctl bootstrap gui/501 "),
        "{ran:?}"
    );
}

#[test]
fn the_desktop_autostart_and_windows_with_a_data_root() {
    let fake = Fake::new(Manager::Autostart);
    flow::run(&fake, ServiceAction::Enable);
    assert!(fake.ran().is_empty(), "自启动不用跑命令");
    let status = flow::run(&fake, ServiceAction::Status);
    assert!(
        matches!(
            status,
            Said::Status {
                installed: true,
                on: None,
                ..
            }
        ),
        "{status:?}"
    );
    let windows = Fake {
        data_root: Some(PathBuf::from("/data")),
        ..Fake::new(Manager::Task)
    };
    assert_eq!(
        flow::run(&windows, ServiceAction::Enable),
        Said::Cannot(Problem::DataRootOnWindows)
    );
    assert!(windows.files.borrow().is_empty(), "装不了的什么都不写");
    let nowhere = Fake {
        home: None,
        ..Fake::new(Manager::Systemd)
    };
    assert_eq!(flow::run(&nowhere, ServiceAction::Enable), Said::NoHome);
}
