//! 装好以后问一句「打开？」（作者说明第九节第 1 条，`docs/blueprint/cli/pkg.md`「怎么走」第 4 步）：装上的包有开关、关着的，
//! 或者开着却没在跑、又要能力的（清单写了一起来就拉起，能力还没批），问「现在打开？[Y/n]」，答是发 `package.enable`，看一眼时
//! 列出的那几个能力一并批；`--yes` 当是。程序不在的不问（打不开）。答不、标准输入关着的不打开，说一句以后在「软件包」页里
//! 打开，照样算装好了。

use std::io::{BufRead, Write};

use serde_json::{Value, json};

use crate::language::Language;
use crate::rpc::Rpc;
use crate::shown::{say, write};

/// 装好了的这一项 `installed`（同 `package.list` 的一项）要不要问，看一眼的回应是 `preview`：有开关、程序在，关着的问；开着
/// 却没在跑、又要能力的也问。
pub(super) fn wanted(installed: &Value, preview: &Value) -> bool {
    let status = installed["status"].as_str().unwrap_or_default();
    if installed.get("enabled").is_none() || status == "program_missing" {
        return false;
    }
    installed["enabled"] == false
        || (!matches!(status, "running" | "starting") && !capabilities(preview).is_empty())
}

/// 看一眼的回应 `preview` 里要的能力的编号。
pub(super) fn capabilities(preview: &Value) -> Vec<String> {
    preview["capabilities"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|capability| capability["id"].as_str())
        .map(str::to_string)
        .collect()
}

/// 问一句、读一行答：空的、`y`、`yes` 打开；别的、标准输入关着的不打开。`yes` 的不问。
pub(super) fn agreed(
    input: &mut dyn BufRead,
    out: &mut dyn Write,
    yes: bool,
    language: &Language,
) -> bool {
    if yes {
        return true;
    }
    write(out, &format!("\n{}", language.open_now()));
    let mut answer = String::new();
    match input.read_line(&mut answer) {
        Ok(0) | Err(_) => {
            say(out, "");
            false
        }
        Ok(_) => matches!(answer.trim().to_lowercase().as_str(), "" | "y" | "yes"),
    }
}

/// 打开包 `id`，批 `approve` 这几个能力。
pub(super) async fn open(
    rpc: &mut Rpc,
    id: &str,
    approve: &[String],
    language: &Language,
    err: &mut dyn Write,
) -> Result<Value, u8> {
    let mut params = json!({"package": id});
    if !approve.is_empty() {
        params["approve"] = json!(approve);
    }
    super::ask(rpc, "package.enable", params, language, err).await
}

#[cfg(test)]
mod tests;
