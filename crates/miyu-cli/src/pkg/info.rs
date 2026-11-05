//! `miyu pkg info`（施工 F-8 下起，IN-2 照设计 `49-装包照pacman.md` 第一节改）：装着的照本地（`-Qi`），没装的照软件源（`-Si`），
//! 写成 pacman 的「键 : 值」。键照最宽的那个按显示宽度补齐（中文一个字两格）；没有值的行不印，「依赖」「推荐」没有的写「无」。
//! 印成几行的 [`rows`] 是纯的，测试里照它算。写的是包目录、包文件的路径的（施工 IN-4，照 pacman 的 `-Qip`）照现在的目录换成
//! 绝对的、发 `package.info {"path"}`，印法同装着的，多一行「无法读取」；`-Qi`、`-Si` 只接编号。

use std::io::Write;

use serde_json::{Map, Value, json};

use miyu_kernel::time::{Timestamp, UtcOffset};

use super::confirm::included;
use super::path::{is_path, joined};
use super::query::size;
use super::{PkgPlan, lines, refused};
use crate::ask::Format;
use crate::config::columns;
use crate::diagnostic;
use crate::exit;
use crate::language::{CameFrom, Doing, InfoKey, Language};
use crate::link;
use crate::rpc::Rpc;
use crate::shown::{offset, say};

/// `package.info` 的参数：`source` 是 `-Si`（`true`）、`-Qi`（`false`），`info` 不写。
pub(crate) fn params(package: &str, source: Option<bool>) -> Value {
    let mut params = json!({ "package": package });
    if let Some(source) = source {
        params["source"] = json!(source);
    }
    params
}

/// 问核心、印：`--format json` 的原样印回应；两处都没有的报「未找到软件包」，退出码 1。写的是路径的看那个包目录、包文件，
/// 装不上的照 `install` 那样报（「无法安装 <写的>：…」）；`-Qi`、`-Si`（`source` 写了的）接路径的是用法错，退出码 2。
pub(super) async fn show(
    rpc: &mut Rpc,
    plan: &PkgPlan,
    (package, source): (&str, Option<bool>),
    (out, err): (&mut dyn Write, &mut dyn Write),
) -> Result<(), u8> {
    let language = &plan.language;
    let file = is_path(package);
    if let (true, Some(source)) = (file, source) {
        diagnostic::error(err, *language, &language.info_takes_an_id(source));
        return Err(exit::MISUSE);
    }
    let asked = match file {
        true => json!({"path": joined(&plan.cwd, package)}),
        false => params(package, source),
    };
    let info = match link::answer(rpc, "package.info", asked, language, err).await? {
        Ok(info) => info,
        Err(error) => {
            let problems = match error["data"]["reason"].as_str() {
                Some("unknown_package") if !file => vec![language.package_not_found(package)],
                _ => refused(&error, file.then_some(package), language),
            };
            for problem in problems {
                diagnostic::error(err, *language, &problem);
            }
            return Err(exit::ERROR);
        }
    };
    match plan.args.format {
        Format::Json => say(out, &info.to_string()),
        Format::Text => lines(out, rows(&info, offset(), language)),
    }
    Ok(())
}

/// `package.info` 的回应印成几行：先后照设计 49 第一节第 3、4 条，键照最宽的那个补齐，冒号前后各空一格。没装的
/// （`from: "source"`）多「软件源」「下载大小」，装着的多「被依赖」「安装时间」。包目录、包文件的（`from: "file"`，施工 IN-4）
/// 照装着的，核心不给来源、被依赖、安装时间，「提供」后面多「无法读取」。
pub(crate) fn rows(info: &Value, offset: UtcOffset, language: &Language) -> Vec<String> {
    let remote = info["from"] == "source";
    let text = |key: &str| info[key].as_str().map(str::to_string);
    let mut rows: Vec<(InfoKey, Option<String>)> = Vec::new();
    if remote {
        rows.push((InfoKey::Repository, text("source")));
    }
    rows.extend([
        (InfoKey::Name, text("package")),
        (InfoKey::Title, text("name")),
        (InfoKey::Version, text("version")),
        (InfoKey::Description, text("summary")),
        (InfoKey::DependsOn, Some(ids(&info["depends"], language))),
        (
            InfoKey::Recommends,
            Some(ids(&info["recommends"], language)),
        ),
        (InfoKey::Provides, provides(info, remote, language)),
        (InfoKey::Skipped, skipped(info, language)),
        (InfoKey::Capabilities, capabilities(info, language)),
        (InfoKey::Origin, came(info, language)),
        (InfoKey::Author, text("author")),
        (InfoKey::Upstream, text("upstream")),
    ]);
    let bytes = |key: &str| info[key].as_u64().map(size);
    if remote {
        rows.push((InfoKey::DownloadSize, bytes("download")));
        rows.push((InfoKey::InstalledSize, bytes("installed_size")));
    } else {
        rows.push((InfoKey::InstalledSize, bytes("size")));
        let required = info["required_by"].as_array().filter(|by| !by.is_empty());
        rows.push((
            InfoKey::RequiredBy,
            required.map(|_| ids(&info["required_by"], language)),
        ));
        rows.push((InfoKey::InstallDate, installed(info, offset, language)));
    }
    aligned(rows, language)
}

/// 几行「键 : 值」（装之前那一份也照它印，施工 IN-3）：没有值的行不印，键照最宽的那个按显示宽度补齐（中文一个字两格），冒号前后
/// 各空一格。值里有换行的（施工 PK-3 下：来源、能力一项一行）后面几行缩进到值的那一列。
pub(super) fn aligned(rows: Vec<(InfoKey, Option<String>)>, language: &Language) -> Vec<String> {
    let rows: Vec<(&str, String)> = rows
        .into_iter()
        .filter_map(|(key, value)| Some((language.info_key(key), value?)))
        .collect();
    let width = rows.iter().map(|(key, _)| columns(key)).max().unwrap_or(0);
    let indent = " ".repeat(width + 4);
    rows.into_iter()
        .flat_map(|(key, value)| {
            let pad = " ".repeat(width + 1 - columns(key));
            let mut parts = value.split('\n');
            let first = format!("{key}{pad} : {}", parts.next().unwrap_or_default());
            let rest: Vec<String> = parts.map(|part| format!("{indent}{part}")).collect();
            std::iter::once(first).chain(rest)
        })
        .collect()
}

/// 几个包的编号，空两格隔开（照 pacman）；一个都没有的「无」。
fn ids(value: &Value, language: &Language) -> String {
    let ids: Vec<&str> = value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    match ids.is_empty() {
        true => language.info_none().to_string(),
        false => ids.join("  "),
    }
}

/// 「提供」：照装之前那一份「包含」的写法（[`included`]）。软件源索引里的（种类到名字）先照从软件源装看一眼的那样摊开：子命令
/// 取第一个，功能、带给别的包的内容不列。
fn provides(info: &Value, remote: bool, language: &Language) -> Option<String> {
    let provides = &info["provides"];
    let items = match remote {
        false => included(provides, Doing::Install, language),
        true => {
            let mut shaped = Map::new();
            for (kind, names) in provides.as_object().into_iter().flatten() {
                match kind.as_str() {
                    "command" => {
                        shaped.insert(kind.clone(), names[0].clone());
                    }
                    "features" | "content" => {}
                    _ => {
                        shaped.insert(kind.clone(), names.clone());
                    }
                }
            }
            included(&Value::Object(shaped), Doing::Install, language)
        }
    };
    (!items.is_empty()).then(|| items.join(language.list_separator()))
}

/// 「无法读取」（施工 IN-4）：读不成的技能、脚本、斜杠命令模板，写法同装之前那一份（`<包里的路径>（<为什么>）`）。
fn skipped(info: &Value, language: &Language) -> Option<String> {
    let items: Vec<String> = info["skipped"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|one| {
            language.skipped(
                one["file"].as_str().unwrap_or_default(),
                one["why"].as_str().unwrap_or_default(),
            )
        })
        .collect();
    (!items.is_empty()).then(|| items.join(language.list_separator()))
}

/// 「需要的能力」：照核心给的名字，同装之前那一份。
pub(super) fn capabilities(info: &Value, language: &Language) -> Option<String> {
    let names: Vec<&str> = info["capabilities"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|capability| capability["name"].as_str())
        .collect();
    (!names.is_empty()).then(|| names.join(language.list_separator()))
}

/// 「来源」：没装的照索引的来源仓库和是不是官方；装着的照核心的 `source`（施工 PK-6：官方、社区、本机文件、出厂、系统装的，
/// 不知道的照实说「不知道」，不说成官方），从软件源装的接来源仓库。连的核心没有 `source` 的照 `from`：它把从软件源装的都记成
/// `official`，分不出是不是官方的源，只说「软件源」。
fn came(info: &Value, language: &Language) -> Option<String> {
    let origin = info["origin"].as_str();
    if info["from"] != "source"
        && let Some(source) = info["source"].as_str()
    {
        let from = match source {
            "official" => CameFrom::Official,
            "community" => CameFrom::Community,
            "file" => CameFrom::Local,
            "shipped" => CameFrom::Shipped,
            "distro" => CameFrom::Distro,
            _ => CameFrom::Unknown,
        };
        return Some(language.came_from(from, origin));
    }
    let from = match info["from"].as_str()? {
        "source" if info["official"] == true => CameFrom::Official,
        "source" => CameFrom::Community,
        "shipped" => CameFrom::Shipped,
        "official" => CameFrom::Source,
        "local" => CameFrom::Local,
        _ => return None,
    };
    Some(language.came_from(from, origin))
}

/// 「安装时间」：照这台机器此刻的时区到分钟；出厂的写「出厂自带」，本地库没记的没有。
fn installed(info: &Value, offset: UtcOffset, language: &Language) -> Option<String> {
    if info["layer"] == "shipped" {
        return Some(language.shipped().to_string());
    }
    let at = info["installed"].as_str()?;
    Some(Timestamp::parse(at).map_or_else(|_| at.to_string(), |at| at.local_minute(offset)))
}

#[cfg(test)]
mod tests;
