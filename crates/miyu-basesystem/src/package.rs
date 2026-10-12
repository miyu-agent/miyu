//! `package`（`docs/blueprint/tools/package.md`，施工 F-10 下，设计 `31-软件包.md` 第六节）：她装、卸软件包。`install` 装一个
//! 包文件夹（绝对路径），`remove` 卸一个装了的包。访问类别 `install`：权限策略照级别判，完全放开直接做，工作区每次问人，
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
    one: Template,
    refused: Template,
    unavailable: Template,
    invalid: Template,
    invalid_line: Template,
    relative: Template,
    unknown: Template,
}

/// 她给的参数：写一个。别的参数不认，也不报错。
#[derive(Deserialize)]
struct Args {
    #[serde(default)]
    install: Option<String>,
    #[serde(default)]
    remove: Option<String>,
}

/// 要做的那一件。
enum Doing {
    Install(PathBuf),
    Remove(String),
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
                one: own("one", &[])?,
                refused: own("refused", &["what", "reason"])?,
                unavailable: own("unavailable", &[])?,
                invalid: shared("invalid", &["path", "problem"])?,
                invalid_line: shared("invalid-line", &["path", "line", "problem"])?,
                relative: shared("relative", &[])?,
                unknown: shared("unknown", &["package"])?,
            },
        })
    }
}

impl Texts {
    /// 照参数认出要做的那一件：两个都写、都没写的，`install` 不是绝对路径的，说哪里不对。
    fn doing(&self, call: &Call) -> Result<Doing, String> {
        let args: Args = serde_json::from_str(&call.args).unwrap_or(Args {
            install: None,
            remove: None,
        });
        match (args.install, args.remove) {
            (Some(path), None) if Path::new(&path).is_absolute() => Ok(Doing::Install(path.into())),
            (Some(_), None) => Err(say(&self.relative, &[])),
            (None, Some(package)) => Ok(Doing::Remove(package)),
            _ => Err(say(&self.one, &[])),
        }
    }

    /// 核心拒了：写错的清单说哪里不对、第几行，没有这个包的照 `packages`，别的说原因代码。
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
            refusal => say(
                &self.refused,
                &[("what", what), ("reason", &refusal.reason)],
            ),
        }
    }

    /// 出错的那一份。
    fn failed(text: String) -> Done {
        Done::error(text).said(said("package/failed"))
    }
}

impl Tool for Package {
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

    /// 问人的卡片上那几格：装哪个文件夹、卸哪个包。
    fn asking(&self, call: &Call) -> Vec<(&'static str, String)> {
        match self.texts.doing(call) {
            Ok(Doing::Install(path)) => vec![("install", path.to_string_lossy().into_owned())],
            Ok(Doing::Remove(package)) => vec![("remove", package)],
            Err(_) => Vec::new(),
        }
    }

    /// 问人之前看一眼：装的同 `package.install` 带 `preview`，卸的同 `package.remove` 带 `preview`。看不了的照拒绝交回她。
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
                Doing::Remove(package) => (port.removing(package.clone()).await, package),
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
                    match port.install(path).await {
                        Ok(entry) => {
                            let package = entry["package"].as_str().unwrap_or_default().to_string();
                            Done::ok(say(&texts.installed, &[("package", &package)]))
                                .said(said("package/installed").with("package", &package))
                        }
                        Err(refusal) => Texts::failed(texts.refused(&refusal, &what)),
                    }
                }
                Doing::Remove(package) => match port.remove(package.clone()).await {
                    Ok(_) => Done::ok(say(&texts.removed, &[("package", &package)]))
                        .said(said("package/removed").with("package", &package)),
                    Err(refusal) => Texts::failed(texts.refused(&refusal, &package)),
                },
            }
        })
    }
}
