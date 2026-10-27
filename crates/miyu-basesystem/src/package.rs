//! `package`（`docs/blueprint/tools/package.md`，施工 F-10 下，设计 `31-软件包.md` 第六节）：她装、卸软件包，打开装着的。
//! `install` 装一个包文件夹（绝对路径，装上以后在会话用的预设里打开，施工 F-10 下下），或者写编号从软件源装（施工 G-6，同样在
//! 预设里打开），`upgrade` 照软件源升级一个包（写 `all` 的是能升的全部，施工 G-6），`remove` 卸一个装了的包，`enable` 打开
//! 装着的扩展、带了 MCP 服务的包（要的能力一并批，施工 F-10 下下）。访问类别 `install`：权限策略照级别判，完全放开直接做，工作区每次问人，
//! 只读拒绝；要问的先看一眼（[`Tool::preview`]，同 `package.install`、`package.remove` 带 `preview` 的），人看的卡片照它。
//! 装的包文件夹报那个路径是读（同 `packages`）：只碰得到自己工作区的会话装不了外面的，数据根哪一级都不许。经
//! [`Call::packages`] 交给核心；写错的清单、读不了的文件夹这些照 `packages` 那几句说。

use std::path::{Path, PathBuf};

use serde::Deserialize;

use miyu_kernel::template::Template;
use miyu_kernel::tool::Access;
use miyu_tool::{
    Call, Done, PACKAGE, PACKAGES, PackageRefusal, Previewing, Progress, Running, Spec, Target,
    Tool,
};

use crate::common::said;
use crate::load::{self, LoadError, say};

/// `upgrade` 写它的是能升的全部。
const ALL: &str = "all";

/// `package`。
pub(crate) struct Package {
    spec: Spec,
    texts: Texts,
}

/// 输出里给她看的几句：自己的在 `software/basesystem/package/`，清单写错、相对路径、没有这个包和 `packages` 共用。
#[derive(Clone)]
struct Texts {
    installed: Template,
    removed: Template,
    enabled: Template,
    one: Template,
    upgraded: Template,
    nothing: Template,
    refused: Template,
    /// 核心拒 `admin_mode_required`（施工 U23 下上）：让人进管理员模式。
    admin_mode: Template,
    unavailable: Template,
    invalid: Template,
    invalid_line: Template,
    relative: Template,
    unknown: Template,
    conflict: Template,
    conflict_line: Template,
}

/// 她给的参数：写一个。别的参数不认，也不报错。
#[derive(Deserialize)]
struct Args {
    #[serde(default)]
    install: Option<String>,
    #[serde(default)]
    remove: Option<String>,
    #[serde(default)]
    enable: Option<String>,
    #[serde(default)]
    upgrade: Option<String>,
}

/// 要做的那一件。
enum Doing {
    Install(PathBuf),
    /// 从软件源装这个编号（施工 G-6）。
    Fetch(String),
    /// 照软件源升级：这一个，没有的是能升的全部（施工 G-6）。
    Upgrade(Option<String>),
    Remove(String),
    Enable(String),
}

impl Package {
    /// 照资源目录 `resources` 里的字造。
    pub(crate) fn load(resources: &Path) -> Result<Package, LoadError> {
        let own = |name: &str, fields: &[&str]| load::text(resources, PACKAGE, name, fields);
        let shared = |name: &str, fields: &[&str]| load::text(resources, PACKAGES, name, fields);
        Ok(Package {
            spec: load::spec(resources, PACKAGE, Access::Install)?,
            texts: Texts {
                installed: own("installed", &["package"])?,
                removed: own("removed", &["package"])?,
                enabled: own("enabled", &["package"])?,
                one: own("one", &[])?,
                upgraded: own("upgraded", &["packages"])?,
                nothing: own("nothing-to-upgrade", &[])?,
                refused: own("refused", &["what", "reason"])?,
                admin_mode: own("admin-mode", &["what"])?,
                unavailable: own("unavailable", &[])?,
                invalid: shared("invalid", &["path", "problem"])?,
                invalid_line: shared("invalid-line", &["path", "line", "problem"])?,
                relative: shared("relative", &[])?,
                unknown: shared("unknown", &["package"])?,
                conflict: shared("conflict", &["path"])?,
                conflict_line: shared("conflict-line", &["kind", "name", "package"])?,
            },
        })
    }
}

impl Texts {
    /// 照参数认出要做的那一件：写了不止一个、一个都没写的，`install` 是相对路径的，说哪里不对。`install` 没有路径分隔符的是
    /// 编号，从软件源装（施工 G-6）。
    fn doing(&self, call: &Call) -> Result<Doing, String> {
        let args: Args = serde_json::from_str(&call.args).unwrap_or(Args {
            install: None,
            remove: None,
            enable: None,
            upgrade: None,
        });
        match (args.install, args.remove, args.enable, args.upgrade) {
            (Some(path), None, None, None) if Path::new(&path).is_absolute() => {
                Ok(Doing::Install(path.into()))
            }
            (Some(id), None, None, None) if !id.contains(['/', '\\']) => Ok(Doing::Fetch(id)),
            (Some(_), None, None, None) => Err(say(&self.relative, &[])),
            (None, Some(package), None, None) => Ok(Doing::Remove(package)),
            (None, None, Some(package), None) => Ok(Doing::Enable(package)),
            (None, None, None, Some(which)) => {
                Ok(Doing::Upgrade(Some(which).filter(|which| which != ALL)))
            }
            _ => Err(say(&self.one, &[])),
        }
    }

    /// 核心拒了：写错的清单说哪里不对、第几行，没有这个包的照 `packages`，撞了名的一处一行，要管理员模式的让人进管理员模式，别的
    /// 说原因代码。
    fn refused(&self, refusal: &PackageRefusal, what: &str) -> String {
        match refusal {
            PackageRefusal {
                problem: Some(problem),
                line: Some(line),
                ..
            } => say(
                &self.invalid_line,
                &[
                    ("path", what),
                    ("line", &line.to_string()),
                    ("problem", problem),
                ],
            ),
            PackageRefusal {
                problem: Some(problem),
                ..
            } => say(&self.invalid, &[("path", what), ("problem", problem)]),
            refusal if refusal.reason == "unknown_package" => {
                say(&self.unknown, &[("package", what)])
            }
            // 系统那一层的事（施工 U23 下上）：她照着告诉人进管理员模式。
            refusal if refusal.reason == "admin_mode_required" => {
                say(&self.admin_mode, &[("what", what)])
            }
            // 撞了名的说清每一处（施工 F-8 六补修）：只给原因代码她改不了自己的包。
            refusal if !refusal.conflicts.is_empty() => crate::packages::conflicting(
                (&self.conflict, &self.conflict_line),
                what,
                &refusal.conflicts,
            ),
            refusal => say(
                &self.refused,
                &[("what", what), ("reason", &refusal.reason)],
            ),
        }
    }

    /// 升好了：升了的一个个列出来（`编号 新版本 (was 旧版本)`），一个都没有的说没有要升的。
    fn upgraded_done(&self, done: &serde_json::Value) -> Done {
        let rows: Vec<String> = done["upgraded"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|row| {
                format!(
                    "{} {} (was {})",
                    row["package"].as_str().unwrap_or_default(),
                    row["to"].as_str().unwrap_or_default(),
                    row["from"].as_str().unwrap_or("?")
                )
            })
            .collect();
        if rows.is_empty() {
            return Done::ok(say(&self.nothing, &[])).said(said("package/nothing-to-upgrade"));
        }
        let packages = rows.join(", ");
        Done::ok(say(&self.upgraded, &[("packages", &packages)]))
            .said(said("package/upgraded").with("packages", &packages))
    }

    /// 出错的那一份。
    fn failed(text: String) -> Done {
        Done::error(text).said(said("package/failed"))
    }
}

impl Tool for Package {
    fn command_only(&self) -> bool {
        true
    }

    fn spec(&self) -> &Spec {
        &self.spec
    }

    /// 装的包文件夹报那个路径是读（同 `packages`）。卸不碰路径。
    fn targets(&self, call: &Call) -> Vec<Target> {
        match self.texts.doing(call) {
            Ok(Doing::Install(path)) => vec![Target {
                path: path.to_string_lossy().into_owned(),
                write: false,
                itself: false,
            }],
            _ => Vec::new(),
        }
    }

    /// 问人的卡片上那几格：装哪个文件夹、卸哪个包、打开哪个包。
    fn asking(&self, call: &Call) -> Vec<(&'static str, String)> {
        match self.texts.doing(call) {
            Ok(Doing::Install(path)) => vec![("install", path.to_string_lossy().into_owned())],
            Ok(Doing::Fetch(package)) => vec![("install", package)],
            Ok(Doing::Upgrade(which)) => {
                vec![("upgrade", which.unwrap_or_else(|| ALL.to_string()))]
            }
            Ok(Doing::Remove(package)) => vec![("remove", package)],
            Ok(Doing::Enable(package)) => vec![("enable", package)],
            Err(_) => Vec::new(),
        }
    }

    /// 问人之前看一眼：装的同 `package.install` 带 `preview`，卸的同 `package.remove` 带 `preview`，打开的看装着的那一份（带了
    /// 什么、要什么能力）。看不了的照拒绝交回她。
    fn preview(&self, call: Call) -> Option<Previewing<'_>> {
        let doing = self.texts.doing(&call).ok()?;
        let port = call.packages?;
        let texts = self.texts.clone();
        Some(Box::pin(async move {
            let (looked, what) = match doing {
                Doing::Install(path) => {
                    let what = path.to_string_lossy().into_owned();
                    (port.inspect(path).await, what)
                }
                Doing::Fetch(package) => (port.fetching(package.clone()).await, package),
                Doing::Upgrade(which) => {
                    let what = which.clone().unwrap_or_else(|| ALL.to_string());
                    (port.upgrading(which).await, what)
                }
                Doing::Remove(package) => (port.removing(package.clone()).await, package),
                Doing::Enable(package) => (port.enabling(package.clone()).await, package),
            };
            looked.map_err(|refusal| Texts::failed(texts.refused(&refusal, &what)))
        }))
    }

    fn run(&self, call: Call, _progress: Progress) -> Running<'_> {
        let texts = self.texts.clone();
        Box::pin(async move {
            let doing = match texts.doing(&call) {
                Ok(doing) => doing,
                Err(text) => return Texts::failed(text),
            };
            let Some(port) = call.packages.clone() else {
                return Texts::failed(say(&texts.unavailable, &[]));
            };
            if call.stop.stopped() {
                return Done::stopped();
            }
            match doing {
                Doing::Install(path) => {
                    let what = path.to_string_lossy().into_owned();
                    let preset = call.ids.as_ref().and_then(|ids| ids.preset.clone());
                    match port.install(path, preset).await {
                        Ok(entry) => {
                            let package = entry["package"].as_str().unwrap_or_default().to_string();
                            Done::ok(say(&texts.installed, &[("package", &package)]))
                                .said(said("package/installed").with("package", &package))
                        }
                        Err(refusal) => Texts::failed(texts.refused(&refusal, &what)),
                    }
                }
                Doing::Fetch(package) => {
                    let preset = call.ids.as_ref().and_then(|ids| ids.preset.clone());
                    match port.fetch(package.clone(), preset).await {
                        Ok(_) => Done::ok(say(&texts.installed, &[("package", &package)]))
                            .said(said("package/installed").with("package", &package)),
                        Err(refusal) => Texts::failed(texts.refused(&refusal, &package)),
                    }
                }
                Doing::Upgrade(which) => {
                    let what = which.clone().unwrap_or_else(|| ALL.to_string());
                    match port.upgrade(which).await {
                        Ok(done) => texts.upgraded_done(&done),
                        Err(refusal) => Texts::failed(texts.refused(&refusal, &what)),
                    }
                }
                Doing::Remove(package) => match port.remove(package.clone()).await {
                    Ok(_) => Done::ok(say(&texts.removed, &[("package", &package)]))
                        .said(said("package/removed").with("package", &package)),
                    Err(refusal) => Texts::failed(texts.refused(&refusal, &package)),
                },
                Doing::Enable(package) => match port.enable(package.clone()).await {
                    Ok(_) => Done::ok(say(&texts.enabled, &[("package", &package)]))
                        .said(said("package/enabled").with("package", &package)),
                    Err(refusal) => Texts::failed(texts.refused(&refusal, &package)),
                },
            }
        })
    }
}
