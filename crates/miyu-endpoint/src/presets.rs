//! 预设（施工 P-2 上，`docs/blueprint/presets.md`）：造会话时照「开会话时指定、个人设置、系统配置」找预设、几层叠好；
//! `preset.list`、`preset.get`。找哪几层、怎么叠在 `miyu_store::presets`。

use miyu_kernel::id::AccountId;
use serde::Deserialize;
use serde_json::{Value, json};

use miyu_config::Words;
use miyu_policy::preset::{Chosen, Problem};
use miyu_session::PresetPlaces;
use miyu_store::human::Human;
use miyu_store::presets::{Found, PresetError, Presets};
use miyu_store::skills::Skill;
use miyu_store::slash::Command as Template;

use crate::Core;
use crate::config::methods::words;
use crate::hello::Peer;
use crate::personas::{label, remove};
use crate::refusal::Refusal;
use crate::settings::PresetSettings;

mod features;
pub(crate) mod write;

const TARGET: &str = "miyu::endpoint";

/// 账号 `account` 看得到的几层预设：出厂、装着的软件包带的（施工 P-11 上）、系统区、它自己的家目录（施工 Y-2 中上，
/// `scope.rs`；原来一律是管理员的家目录）。
pub(crate) fn presets(core: &Core, account: &AccountId) -> Presets {
    let scope = crate::scope::scope(core, account);
    Presets::new(&core.resources, &core.root, &scope.user)
        .with_packages(&crate::packages::folders(core, account))
}

/// 找新会话的预设：指定了的是它，不然照这时的 `preset.default`（个人设置压着系统配置，都没写的是出厂的 `full`）。指着没有
/// 的不悄悄换成别的（Y12）。在阻塞线程里读盘。
pub(crate) async fn resolve(
    core: &Core,
    account: &AccountId,
    wanted: Option<&str>,
) -> Result<Found, Refusal> {
    let id = match wanted {
        Some(id) => id.to_string(),
        None => PresetSettings::from(&core.mine(account).config().resolved().values()).default,
    };
    let presets = presets(core, account);
    let found = tokio::task::spawn_blocking(move || presets.find(&id)).await;
    match found {
        Ok(Ok(found)) => Ok(found),
        Ok(Err(error)) => Err(refusal(&error)),
        Err(_) => Err(Refusal::INTERNAL),
    }
}

/// 交给属主是 `owner` 的会话的预设的几层和装了的功能（施工 P-2 下；施工 F-3 上起是功能；施工 Y-2 中上起照属主）：改了预设的
/// 文件，开着的会话下一个回合换上。
pub(crate) fn places(core: &Core, owner: &AccountId) -> PresetPlaces {
    PresetPlaces {
        presets: presets(core, owner),
        features: crate::scripts::features(core, owner),
    }
}

/// 找好的预设换成造会话要的：带上这时的全局开关（施工 X-27），算好装了、没开的功能（施工 F-3 上）。
pub(crate) fn chosen(core: &Core, owner: &AccountId, found: Found) -> Chosen {
    let features = crate::scripts::features(core, owner);
    // 这个人关了的包扣下（施工 U23 末上）。
    let file = found
        .file
        .following(switches(core, owner))
        .switching_off(core.packages_off(owner), &features);
    Chosen::new(found.id, file, &features)
}

/// 这时 `account` 的配置里写了的全局开关（施工 X-27，`miyu_session::switches`；施工 Y-2 下下中上起照人）：照配置服务最近换上的
/// 那一份读，不拿配置服务的锁。
pub(crate) fn switches(
    core: &Core,
    account: &AccountId,
) -> std::collections::BTreeMap<String, bool> {
    let now = core.mine(account).hub().current();
    miyu_session::switches::of(&now.borrow().resolved().values())
}

/// 找预设出的错照协议说：编号不合写法的参数不对，哪一层都没有的 `unknown_preset`，写错了的 `preset_invalid`（`data.problem`
/// 写明哪一层、哪个文件第几行），读不了的是内部出错。
fn refusal(error: &PresetError) -> Refusal {
    match error {
        PresetError::BadId(_) => Refusal::BAD_PARAMS,
        PresetError::NotFound(_) => Refusal::UNKNOWN_PRESET,
        PresetError::Invalid(..) => Refusal::preset_invalid(error.to_string()),
        PresetError::Unreadable(..) => {
            tracing::warn!(target: TARGET, error = %error, "preset unreadable");
            Refusal::INTERNAL
        }
    }
}

/// 同 [`refusal`]，写错了的多带一句照 `words` 的语言的 `message` 和第几行（施工 P-3 补）：`preset.get`、`preset.set` 用。
pub(crate) fn told(error: &PresetError, words: Option<&Human>) -> Refusal {
    let refused = refusal(error);
    match (error, words) {
        (PresetError::Invalid(_, _, problem), Some(words)) => {
            refused.telling(sentence(problem, words), problem.line)
        }
        _ => refused,
    }
}

/// 写错了的那一处照 `words` 的语言说（施工 P-3 补；再补起 `preset.list` 也用）。
fn sentence(problem: &Problem, words: &Human) -> Option<String> {
    let key = format!("preset-problems/{}", problem.code.as_str());
    Words::sentence(words, &key, &[("detail", problem.detail.as_str())])
}

/// 列表里写错了的一项（施工 P-3 再补）：`problem` 照 `words` 的语言说，知道第几行的带 `line`，同 `data.message`、`data.line`；
/// 没有字的、读不了的照原话。
fn listed_problem(id: &str, error: &PresetError, words: Option<&Human>) -> Value {
    let mut item = json!({"preset": id, "problem": error.to_string()});
    if let (PresetError::Invalid(_, _, problem), Some(words)) = (error, words) {
        if let Some(message) = sentence(problem, words) {
            item["problem"] = json!(message);
        }
        if let Some(line) = problem.line {
            item["line"] = json!(line);
        }
    }
    item
}

/// `preset.list`：几层里所有的预设，照编号排。每个带名字、说明（照这个连接的语言挑）；写错了的带 `problem`（照这个连接的
/// 语言说，知道第几行的带 `line`，施工 P-3 再补）、不带名字和说明。来自哪几层不往外给（施工 P-3 补）。
pub(crate) async fn list(core: &Core, account: &AccountId, peer: Peer) -> Result<Value, Refusal> {
    let said = words(core, peer.language).ok();
    let presets = presets(core, account);
    let read = tokio::task::spawn_blocking(move || {
        presets
            .ids()
            .into_iter()
            .map(|id| {
                let found = presets.find(&id);
                (id, found)
            })
            .collect::<Vec<_>>()
    })
    .await
    .map_err(|_| Refusal::INTERNAL)?;
    let listed: Vec<Value> = read
        .into_iter()
        .map(|(id, found)| match found {
            Ok(found) => json!({
                "preset": id,
                "name": label(found.file.name.as_ref(), peer.language),
                "summary": label(found.file.summary.as_ref(), peer.language),
                "icon": found.file.icon,
            }),
            Err(error) => listed_problem(&id, &error, said.as_ref()),
        })
        .collect();
    Ok(json!({"presets": listed}))
}

/// 开会话的 `preset`（施工 P-16 下）：没写的照默认（没有），写 `null` 的是无工具，内部记成空的编号（同换预设时内核记的）。
///
/// # Errors
///
/// 写了空字：`bad_params`（空的编号留给无工具）。
pub(crate) fn opening(preset: Option<Option<String>>) -> Result<Option<String>, Refusal> {
    match preset {
        Some(Some(id)) if id.is_empty() => Err(Refusal::BAD_PARAMS),
        Some(id) => Ok(Some(id.unwrap_or_default())),
        None => Ok(None),
    }
}

/// 会话信息里写预设（施工 P-16 下）：无工具的（内部是空的编号）写 `preset: null`、`no_tools: true`，别的照编号。
pub(crate) fn shown(reply: &mut Value, preset: &str) {
    if preset.is_empty() {
        reply["preset"] = Value::Null;
        reply["no_tools"] = json!(true);
    } else {
        reply["preset"] = json!(preset);
    }
}

/// `preset.get` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GetParams {
    preset: String,
}

/// `preset.get`：叠好的样子（施工 P-3 补：只给人要看的）。名字、说明照这个连接的语言挑；`unlisted` 是叠好以后的（几层都没写的
/// 是 `on`）；`features` 是一个个功能，各带下面的工具（施工 F-3 下，`features.rs`）；删了会怎样（`remove`，同人格）。来自哪几层
/// 不往外给。
pub(crate) async fn get(
    core: &Core,
    account: &AccountId,
    peer: Peer,
    params: GetParams,
) -> Result<Value, Refusal> {
    let said = words(core, peer.language).ok();
    let presets = presets(core, account);
    let found = tokio::task::spawn_blocking(move || presets.find(&params.preset))
        .await
        .map_err(|_| Refusal::INTERNAL)?
        .map_err(|error| told(&error, said.as_ref()))?;
    let carried = carried(core, account).await;
    Ok(describe(core, &found, peer.language, &carried))
}

/// 一个账号用得上的技能（施工 X-11 上）、模板命令（施工 X-14 上）：功能表照它们一个列一个功能（施工 Y-2 中上起照调用者，原来是
/// 管理员的）。
pub(super) struct Carried {
    /// 谁的（施工 Y-2 下下中上：功能表的全局开关照他）。
    pub(super) account: AccountId,
    pub(super) skills: Vec<Skill>,
    pub(super) commands: Vec<Template>,
    /// 扩展声明的命令（施工 X-14 下）。
    pub(super) declared: Vec<crate::commands::Declared>,
}

/// 找 [`Carried`]：碰磁盘，在阻塞线程里找；找不成的当没有。
pub(super) async fn carried(core: &Core, account: &AccountId) -> Carried {
    let (packages, root, admin) = (
        core.packages_of(account),
        core.root.clone(),
        account.clone(),
    );
    let skills =
        tokio::task::spawn_blocking(move || miyu_store::skills::find(&packages, &root, &admin).0)
            .await
            .unwrap_or_default();
    Carried {
        account: account.clone(),
        skills,
        commands: crate::commands::template_commands(core, account).await,
        declared: crate::commands::declared_commands(core),
    }
}

/// 叠好的一个预设照 `preset.get` 写（`preset.set` 的回应也是它，施工 P-3 中），名字、说明照语言 `language` 挑。
fn describe(core: &Core, found: &Found, language: &str, carried: &Carried) -> Value {
    json!({
        "preset": found.id,
        "name": label(found.file.name.as_ref(), language),
        "summary": label(found.file.summary.as_ref(), language),
        "icon": found.file.icon,
        "unlisted": found.file.unlisted().as_str(),
        "features": features::listed(core, found, language, carried),
        "remove": remove(&found.layers),
    })
}
