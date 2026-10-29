//! `miyu pkg info`（施工 F-8 下起，IN-2 照设计 `49-装包照pacman.md` 第一节改）：装着的照本地（`-Qi`），没装的照软件源（`-Si`），
//! 写成 pacman 的「键 : 值」。键照最宽的那个按显示宽度补齐（中文一个字两格）；没有值的行不印，「依赖」「推荐」没有的写「无」。
//! 印成几行的 [`rows`] 是纯的，测试里照它算。

use std::io::Write;

use serde_json::{Map, Value, json};

use miyu_kernel::time::{Timestamp, UtcOffset};

use super::confirm::included;
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

/// 问核心、印：`--format json` 的原样印回应；两处都没有的报「未找到软件包」，退出码 1。
pub(super) async fn show(
    rpc: &mut Rpc,
    plan: &PkgPlan,
    (package, source): (&str, Option<bool>),
    (out, err): (&mut dyn Write, &mut dyn Write),
) -> Result<(), u8> {
    let language = &plan.language;
    let asked = params(package, source);
    let info = match link::answer(rpc, "package.info", asked, language, err).await? {
        Ok(info) => info,
        Err(error) => {
            let problems = match error["data"]["reason"].as_str() {
                Some("unknown_package") => vec![language.package_not_found(package)],
                _ => refused(&error, None, language),
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
/// （`from: "source"`）多「软件源」「下载大小」，装着的多「被依赖」「安装时间」。
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
    let rows: Vec<(&str, String)> = rows
        .into_iter()
        .filter_map(|(key, value)| Some((language.info_key(key), value?)))
        .collect();
    let width = rows.iter().map(|(key, _)| columns(key)).max().unwrap_or(0);
    rows.into_iter()
        .map(|(key, value)| {
            let pad = " ".repeat(width + 1 - columns(key));
            format!("{key}{pad} : {value}")
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

/// 「需要的能力」：照核心给的名字，同装之前那一份。
fn capabilities(info: &Value, language: &Language) -> Option<String> {
    let names: Vec<&str> = info["capabilities"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|capability| capability["name"].as_str())
        .collect();
    (!names.is_empty()).then(|| names.join(language.list_separator()))
}

/// 「来源」：没装的照索引的来源仓库和是不是官方；装着的照 `from`，从软件源装的接来源仓库。装着的分不出装自官方的源还是别人的
/// 源（核心的 `from` 把从软件源装的都记成 `official`），只说「软件源」，不说官方。
fn came(info: &Value, language: &Language) -> Option<String> {
    let origin = info["origin"].as_str();
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
