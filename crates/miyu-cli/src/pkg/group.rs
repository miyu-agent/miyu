//! 装回、卸掉整个组（施工 BS-1 下中，`docs/blueprint/cli/pkg.md`，照 pacman 写组名）：`miyu pkg install basesystem`、
//! `miyu pkg remove basesystem`。核心看一眼回 `{"group", "packages": [<编号>…]}` 的就是组：没写 `--yes` 的印要动哪几个、问一句，
//! 做完一个一行说装回了、卸掉了谁。组里没有要动的说一句，退出码 0。单个包的卸照旧（[`remove`]）。

use std::io::{BufRead, Write};

use serde_json::{Value, json};

use super::{PkgPlan, ask, confirm, lines, package_of, previewed};
use crate::language::Doing;
use crate::rpc::Rpc;
use crate::shown::say;

/// 看一眼的回应是不是一个组的。
pub(super) fn is_group(shown: &Value) -> bool {
    shown.get("group").is_some()
}

/// 照看一眼的 `shown` 动一个组：`method` 是 `package.install` 或 `package.remove`，`params` 是不带 `preview` 的参数。
pub(super) async fn run(
    rpc: &mut Rpc,
    plan: &PkgPlan,
    (method, params, shown): (&str, Value, &Value),
    (input, out, err): (&mut dyn BufRead, &mut dyn Write, &mut dyn Write),
) -> Result<(), u8> {
    let language = &plan.language;
    let doing = match method {
        "package.remove" => Doing::Remove,
        _ => Doing::Restore,
    };
    let group = shown["group"].as_str().unwrap_or_default();
    if shown["packages"].as_array().is_none_or(Vec::is_empty) {
        say(out, &language.nothing_in_group(group));
        return Ok(());
    }
    if !plan.args.yes {
        lines(out, confirm::group_plan(shown, doing, language));
        confirm::asked(input, out, err, doing, language)?;
    }
    let result = ask(rpc, method, params, language, err).await?;
    let done = match doing {
        Doing::Remove => result["removed"].clone(),
        _ => result["packages"].clone(),
    };
    for one in done.as_array().into_iter().flatten() {
        let id = one.as_str().unwrap_or_else(|| package_of(one));
        let line = match doing {
            Doing::Remove => language.uninstalled(id),
            _ => language.installed(id),
        };
        say(out, &line);
    }
    Ok(())
}

/// `miyu pkg remove <编号或组>`：先看一眼（写的是组的照 [`run`]），没写 `--yes` 的印出来、问一句，卸掉。
pub(super) async fn remove(
    rpc: &mut Rpc,
    plan: &PkgPlan,
    package: &str,
    (input, out, err): (&mut dyn BufRead, &mut dyn Write, &mut dyn Write),
) -> Result<(), u8> {
    let language = &plan.language;
    let params = json!({"package": package});
    let shown = ask(rpc, "package.remove", previewed(&params), language, err).await?;
    if is_group(&shown) {
        return run(
            rpc,
            plan,
            ("package.remove", params, &shown),
            (input, out, err),
        )
        .await;
    }
    if !plan.args.yes {
        lines(out, confirm::plan(&shown, Doing::Remove, language));
        confirm::asked(input, out, err, Doing::Remove, language)?;
    }
    let result = ask(rpc, "package.remove", params, language, err).await?;
    say(out, &language.uninstalled(package_of(&result)));
    Ok(())
}
