//! 装、卸之前印的那一份和问的那一句（施工 F-8 下补，`docs/blueprint/cli/pkg.md`「怎么走」第 3 步，照 pacman）：核心看一眼的
//! 回应（`package.install`、`package.remove` 带 `preview`）印成几行，空一行问「继续？[Y/n]」，读一行答：空的、`y`、`yes`
//! 接着做，别的不做；标准输入关着的（脚本里）也不做，提示用 `--yes`。

use std::io::{BufRead, Write};

use serde_json::Value;

use super::approve;
use super::query::size;
use crate::config::columns;
use crate::diagnostic;
use crate::exit;
use crate::language::{Carried, Doing, Language, PlanLabel};
use crate::shown::{say, write};

/// 装回、卸掉整个组看一眼的那一份（施工 BS-1 下中，核心回 `{"group", "packages": [<编号>…]}`）：一行说动哪个组，一行列这一次
/// 会动的包。
pub(crate) fn group_plan(preview: &Value, doing: Doing, language: &Language) -> Vec<String> {
    let ids: Vec<&str> = preview["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    vec![
        language.will(doing, preview["group"].as_str().unwrap_or_default()),
        format!(
            "{}{}",
            language.plan_label(PlanLabel::Includes),
            ids.join(language.list_separator())
        ),
    ]
}

/// 看一眼的回应 `preview` 印成几行：哪个包（版本、换下哪一份）、包含什么、读不成的（施工 X-13 补）、需要什么能力、一并删除
/// 什么、会删掉的状态目录（施工 PK-4）、配置目录留下还是删掉（施工 PK-5）、多大。没有的那一行不印。
pub(crate) fn plan(preview: &Value, doing: Doing, language: &Language) -> Vec<String> {
    let separator = language.list_separator();
    let mut head = language.will(doing, preview["package"].as_str().unwrap_or_default());
    if let Some(version) = preview["version"].as_str() {
        head.push(' ');
        head.push_str(version);
    }
    if let Some(replaces) = preview.get("replaces") {
        head.push_str(&language.replacing(replaces["version"].as_str()));
    }
    let mut rows = vec![head];
    let row = |label: PlanLabel, items: Vec<String>| {
        (!items.is_empty())
            .then(|| format!("{}{}", language.plan_label(label), items.join(separator)))
    };
    rows.extend(row(PlanLabel::Includes, included(preview, doing, language)));
    // 驱动会让供应商自己跑的工具（施工 HT-4）：声明了的驱动一个一行，缩进到「包含：」后面那一列；没声明的不印。
    if let Some(hosted) = preview["hosted"].as_object() {
        let indent = " ".repeat(columns(language.plan_label(PlanLabel::Includes)));
        for driver in strings(&preview["drivers"]) {
            let names = hosted
                .get(driver)
                .map(|names| strings(names).join(separator))
                .unwrap_or_default();
            if !names.is_empty() {
                rows.push(format!("{indent}{}", language.hosted_by(driver, &names)));
            }
        }
    }
    // 读不成、装上也用不了的技能、脚本（施工 X-13 补）：包文件夹里的路径和为什么（英文一句，同 `miyu check`）。
    let skipped = preview["skipped"]
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
    rows.extend(row(PlanLabel::Skipped, skipped));
    // 能力一项一行、名字接说明（施工 PK-3 下，设计 61 第三节第 1 条）：后面几行缩进到「需要：」后面那一列。
    let label = language.plan_label(PlanLabel::Needs);
    for (index, need) in approve::needs(preview, false, language).iter().enumerate() {
        match index {
            0 => rows.push(format!("{label}{need}")),
            _ => rows.push(format!("{}{need}", " ".repeat(columns(label)))),
        }
    }
    if doing == Doing::Remove {
        let keys: Vec<&str> = strings(&preview["settings"]);
        let mut deletes = Vec::new();
        if !keys.is_empty() {
            deletes.push(language.deleted_settings(&keys.join(separator)));
        }
        // 旧的核心只说有没有状态目录（`true`），照旧列一项。
        if preview["state"] == true {
            deletes.push(language.state_dir().to_string());
        }
        rows.extend(row(PlanLabel::AlsoDeletes, deletes));
        // 状态目录在哪、多大（施工 PK-4）：单起一行，好让人看清卸掉会丢什么。
        if let Some(path) = preview["state"]["path"].as_str() {
            let bytes = preview["state"]["size"].as_u64().unwrap_or_default();
            rows.push(language.deletes_state(path, &size(bytes)));
        }
        // 配置目录、配置里它的几项设置（施工 PK-5、PK-5补）：照 pacman 默认留下，写了 `--purge` 的删掉；核心没说的（没东西、
        // 旧的核心）不印。
        let config = &preview["config"];
        let kept = config["kept"].as_bool().unwrap_or(true);
        let settings = config["settings"].as_u64().unwrap_or_default();
        rows.extend(language.config_dir(config["path"].as_str(), settings, kept));
    }
    // 从软件源装的（施工 G-4 下）：一起装的依赖、要下多大、来源仓库和官方还是社区。
    let with = preview["with"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|one| {
            format!(
                "{} {}",
                one["package"].as_str().unwrap_or_default(),
                one["version"].as_str().unwrap_or_default()
            )
        })
        .collect();
    rows.extend(row(PlanLabel::With, with));
    if let Some(bytes) = preview["download"].as_u64() {
        rows.extend(row(PlanLabel::Download, vec![size(bytes)]));
    }
    if let Some(origin) = preview["origin"].as_str() {
        let official = preview["official"].as_bool() == Some(true);
        rows.extend(row(
            PlanLabel::From,
            vec![format!("{origin}{}", language.origin_mark(official))],
        ));
    }
    // 不经软件源装的从哪来（施工 PK-3 下）：本机文件、出厂、系统的包管理器；路径是人自己敲的，不再印。
    if let Some(said) = preview["source"]
        .as_str()
        .and_then(|one| language.local_source(one))
    {
        rows.extend(row(PlanLabel::From, vec![said.to_string()]));
    }
    if let (Some(files), Some(bytes)) = (preview["files"].as_u64(), preview["size"].as_u64()) {
        rows.extend(row(
            PlanLabel::Size,
            vec![language.package_size(&size(bytes), files)],
        ));
    }
    rows
}

/// 「包含」那一行（`info` 的「提供」也照它说，施工 IN-2）：程序、子命令、后台页、吉祥物、接的平台、MCP 服务、技能、脚本、几项设置（卸的时候设置算在「一并
/// 删除」里）。
pub(crate) fn included(preview: &Value, doing: Doing, language: &Language) -> Vec<String> {
    let mut items = Vec::new();
    if let Some(kind) = preview["program"].as_str() {
        items.push(language.program(kind));
    }
    let mut carried = |what: Carried<'_>| items.push(language.carried(what));
    if let Some(name) = preview["command"].as_str() {
        carried(Carried::Command(name));
    }
    if preview["page"] == true {
        carried(Carried::Page);
    }
    if preview["mascot"] == true {
        carried(Carried::Mascot);
    }
    if preview["theme"] == true {
        carried(Carried::Theme);
    }
    if let Some(platform) = preview["connection"].as_str() {
        carried(Carried::Connection(platform));
    }
    for name in strings(&preview["mcp"]) {
        carried(Carried::Mcp(name));
    }
    for name in strings(&preview["drivers"]) {
        carried(Carried::Driver(name));
    }
    for name in strings(&preview["skills"]) {
        carried(Carried::Skill(name));
    }
    for name in strings(&preview["scripts"]) {
        carried(Carried::Script(name));
    }
    for name in strings(&preview["slash"]) {
        carried(Carried::Slash(name));
    }
    for name in strings(&preview["personas"]) {
        carried(Carried::Persona(name));
    }
    for name in strings(&preview["presets"]) {
        carried(Carried::Preset(name));
    }
    for name in strings(&preview["knowledge"]) {
        carried(Carried::Knowledge(name));
    }
    if let Some(n) = preview["settings"]
        .as_u64()
        .filter(|_| doing != Doing::Remove)
    {
        carried(Carried::Settings(n));
    }
    items
}

/// 装、装回之前问的那一句（施工 PK-3 下）：装的、核心认 `approve`、有要批的能力的问「安装 '<编号>' 并批准它要的能力？」，别的照旧；
/// 核心给了 `notice` 的前面先一行「注意：…」。
pub(crate) fn question(preview: &Value, doing: Doing, id: &str, language: &Language) -> String {
    let approving =
        doing == Doing::Install && approve::understood(preview) && approve::asks(&[preview]);
    let asked = match approving {
        true => language.proceed_approving(doing, id),
        false => language.proceed(doing, id),
    };
    approve::noticed(approve::notice(&preview["notice"], language), asked)
}

fn strings(value: &Value) -> Vec<&str> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect()
}

/// 空一行问 `question`（[`Language::proceed`] 那一句）、读一行答。接着做的 `Ok`；不做的在标准错误上说一句，交回退出码 1。
pub(crate) fn asked(
    (input, out, err): (&mut dyn BufRead, &mut dyn Write, &mut dyn Write),
    question: &str,
    language: &Language,
) -> Result<(), u8> {
    write(out, &format!("\n{question}"));
    let mut answer = String::new();
    match input.read_line(&mut answer) {
        Ok(0) | Err(_) => {
            say(out, "");
            diagnostic::error(err, *language, &language.unconfirmed());
            Err(exit::ERROR)
        }
        Ok(_) => match answer.trim().to_lowercase().as_str() {
            "" | "y" | "yes" => Ok(()),
            _ => {
                say(err, language.cancelled());
                Err(exit::ERROR)
            }
        },
    }
}

/// 接着问 `question`（不空行，施工 IN-3：同时装推荐的），读一行答：空的、`y`、`yes` 是「是」；别的、标准输入关着的是「否」，
/// 照样往下做。
pub(crate) fn agreed((input, out): (&mut dyn BufRead, &mut dyn Write), question: &str) -> bool {
    write(out, question);
    let mut answer = String::new();
    match input.read_line(&mut answer) {
        Ok(0) | Err(_) => {
            say(out, "");
            false
        }
        Ok(_) => matches!(answer.trim().to_lowercase().as_str(), "" | "y" | "yes"),
    }
}

#[cfg(test)]
mod tests;
