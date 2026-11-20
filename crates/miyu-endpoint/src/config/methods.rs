//! 协议上的 `config.schema`、`config.get`、`config.check`（`internal/blueprint/config.md`「协议」，施工 8-2）：都是查询，
//! 不改什么。说成话、挑出几项的几个小函数 `config.set`、`config.trust` 也用（施工 8-3）。给人看的字（名字、说明、报错的话）照这个连接的语言（握手时定的）。
//!
//! 写了清单里没有的键：`unknown_config_key`，`data.problems` 里每个不认识的一条，带离得最近的键名。

use serde::Deserialize;
use serde_json::{Map, Value, json};

use miyu_config::merge::{Layers, Trust, below, explain, merge};
use miyu_config::problem::{Code, Problem, Told, Using, nearest, tell};
use miyu_config::{Item, Layer, Words};
use miyu_store::human::{FALLBACK, Human};

use super::file::File;
use super::options::{noted, usable};
use super::schema_item::schema_item;
use super::{Config, Project, TARGET, wire};
use crate::Core;
use crate::hello::Peer;
use crate::refusal::Refusal;

/// `config.schema` 的参数。
#[derive(Debug, Deserialize)]
pub(crate) struct SchemaParams {
    /// 只要这几项；不写是全部。
    #[serde(default)]
    keys: Option<Vec<String>>,
}

/// `config.get` 的参数。
#[derive(Debug, Deserialize)]
pub(crate) struct GetParams {
    /// 只要这几项；不写是全部。
    #[serde(default)]
    keys: Option<Vec<String>>,
    /// 照这个目录找项目配置；不写不算项目配置。
    #[serde(default)]
    cwd: Option<String>,
    /// 每一项再列出写了它的每一层。
    #[serde(default)]
    all: bool,
}

/// `config.schema`：配置清单，名字和说明照这个连接的语言。有开关的包多一项「启用」（施工 UX-1，`enabled.rs`）。
pub(crate) fn schema(core: &Core, peer: Peer, params: SchemaParams) -> Result<Value, Refusal> {
    let words = words(core, peer.language)?;
    // 包的开关拿配置的锁之前读。
    let (switches, keys) =
        super::enabled::split(&super::enabled::switches(core), params.keys.as_deref());
    let config = &*core.config();
    let mut items: Vec<&Item> = Vec::new();
    for (item, _) in selected(config, keys.as_deref(), &words)? {
        if !items.iter().any(|seen| seen.key == item.key) {
            items.push(item);
        }
    }
    super::enabled::place(&mut items, &switches);
    // 英文的那一份（施工 NS-1）：这种语言没有的名字照它补，另给每一项、每一组一个英文名 `name_en`，头拿它一起搜（网页 W-1206：
    // 中文界面搜 web 搜不到「网络搜索」）。连接就是英文的不另给。
    let english = (!peer.language.starts_with(FALLBACK))
        .then(|| self::words(core, FALLBACK).ok())
        .flatten();
    let mut pages: Vec<Value> = Vec::new();
    let mut groups: Vec<Value> = Vec::new();
    let mut listed = Vec::new();
    let local = core
        .memory
        .vectors()
        .and_then(|vectors| vectors.local_name());
    let values = config.resolved().values();
    // 这台机器装了哪些界面（施工 UH-1）：只对装了某个界面才有用的项（终端集成会话那几项、默认界面）照它标不画。
    let heads = super::unused::heads(&core.system_packages());
    // 包的分节（施工 SG-1）：组里的项带 `section`，这个包那一组带 `sections`，没分组的包都不写。
    let sections = super::sections::Sections::new(core, peer.language);
    // 选项由包在运行时给的项（施工 SG-2）：带 `choices_from`，没写的不带。
    let choices = super::choices::Choices::new(core);
    for item in items {
        let said = words
            .item(item.key)
            .or_else(|| english.as_ref().and_then(|english| english.item(item.key)));
        let notes =
            |option: &str| noted(core, (&words, &values), item.key, option, local.as_deref());
        let usable = |option: &str| usable(item.key, option, local.is_some());
        let mut one = schema_item(item, said, &notes, &usable);
        if super::unused::unused(item.key, &heads) {
            one["hidden"] = json!(true);
        }
        if let Some(section) = sections.item(item.key) {
            one["section"] = json!(section);
        }
        if let Some(from) = choices.item(item.key) {
            one["choices_from"] = from.clone();
        }
        if let Some(said) = english.as_ref().and_then(|english| english.item(item.key))
            && one["name"] != said.name.as_str()
        {
            one["name_en"] = json!(said.name);
        }
        listed.push(one);
        let page = item.ui.page;
        if !pages.iter().any(|seen| seen["id"] == page) {
            let name = config_name(&words, english.as_ref(), "pages", page);
            pages.push(json!({"id": page, "name": name}));
        }
        let group = item.ui.group;
        if !groups.iter().any(|seen| seen["id"] == group) {
            let name = config_name(&words, english.as_ref(), "groups", group);
            let mut entry = json!({"id": group, "name": name, "page": page});
            if let Some(english) = english.as_ref().and_then(|english| english.group(group))
                && name != english
            {
                entry["name_en"] = json!(english);
            }
            // 组的说明（施工 HZ-1）：资源里写了的才带；这种语言没有的照英文，同组名。
            let described = |human: &Human| human.group_description(group).map(str::to_string);
            if let Some(said) = described(&words).or_else(|| english.as_ref().and_then(described)) {
                entry["description"] = json!(said);
            }
            if let Some(listed) = item
                .ui
                .package
                .and_then(|package| sections.package(package))
            {
                entry["sections"] = listed.clone();
            }
            groups.push(entry);
        }
    }
    Ok(json!({"groups": groups, "items": listed, "pages": pages}))
}

/// 页、组的名字：这种语言没有的照英文，都没有的照编号。
fn config_name(words: &Human, english: Option<&Human>, what: &str, id: &str) -> String {
    let find = |human: &Human| match what {
        "pages" => human.page(id).map(str::to_string),
        _ => human.group(id).map(str::to_string),
    };
    find(words)
        .or_else(|| english.and_then(find))
        .unwrap_or_else(|| id.to_string())
}

/// `config.get`：最终值，每个值附上来源；每一份文件的位置、版本；这几份文件现在的全部问题。
pub(crate) fn get(core: &Core, peer: Peer, params: GetParams) -> Result<Value, Refusal> {
    let words = words(core, peer.language)?;
    let (switches, keys) =
        super::enabled::split(&super::enabled::switches(core), params.keys.as_deref());
    let config = &*core.config();
    let project = params.cwd.as_deref().and_then(|cwd| config.project(cwd));
    let layers = config.layers(project.as_ref());
    let resolved = merge(config.items(), &layers, &|name| {
        config.env.get(name).cloned()
    });
    let shown = |layer: Layer| match layer {
        Layer::System => Some(config.system.shown.clone()),
        Layer::Project => project.as_ref().map(|project| project.file.shown.clone()),
    };
    let wanted = selected_in(config, &resolved, keys.as_deref(), &words)?;
    let values = resolved.values();
    let mut listed = Map::new();
    super::enabled::got(&mut listed, &switches, params.all);
    for (item, key) in wanted {
        let Some((value, origin)) = resolved.get(&key) else {
            // 哪儿都没写、借得到的（施工 NS-1）照样列。
            if let Some(origin) = super::inherited::origin(&key, None, &values) {
                listed.insert(key, json!({"origin": origin, "value": null}));
            }
            continue;
        };
        let mut entry = Map::new();
        let origin = wire::origin_of(&key, origin, &shown, &values);
        entry.insert("origin".to_string(), origin);
        entry.insert("value".to_string(), value.json());
        if params.all {
            let rows: Vec<Value> = explain(item, &key, &layers, &resolved)
                .iter()
                .map(|row| {
                    let mut map = Map::new();
                    map.insert("origin".to_string(), wire::origin(&row.origin, &shown));
                    map.insert("value".to_string(), row.value.json());
                    map.insert("used".to_string(), json!(row.used));
                    if let Some(problem) = row.problem {
                        map.insert("problem".to_string(), json!(problem.as_str()));
                    }
                    Value::Object(map)
                })
                .collect();
            entry.insert("layers".to_string(), json!(rows));
        }
        listed.insert(key, Value::Object(entry));
    }
    let mut problems = Vec::new();
    let file = &config.system;
    let missing = config.missing(&file.parsed, file.layer);
    for problem in file.problems().chain(&missing) {
        problems.push(said(config, &layers, problem, Some(file), &words)?);
    }
    let secrets = &config.secrets;
    for problem in secrets.problems() {
        let place = Some((secrets.shown.as_str(), secrets.last_good));
        problems.push(said_at(config, &layers, problem, place, &words)?);
    }
    if let Some(project) = &project {
        let merged = resolved.problems.iter();
        let missing = config.missing(&project.file.parsed, Layer::Project);
        for problem in project.file.problems().chain(merged).chain(&missing) {
            problems.push(said(config, &layers, problem, Some(&project.file), &words)?);
        }
    }
    Ok(json!({
        "files": files(config, project.as_ref()),
        "items": listed,
        "problems": problems,
    }))
}

/// `files`：每一层的文件在哪、版本；项目配置多一格信不信任；密钥文件只说在哪（施工 8-5：它的版本是整份密钥的哈希，
/// 不交出去）。
fn files(config: &Config, project: Option<&Project>) -> Value {
    let mut files = Map::new();
    let file = &config.system;
    files.insert(
        file.layer.as_str().to_string(),
        json!({"file": file.shown, "version": file.version}),
    );
    files.insert("secrets".to_string(), json!({"file": config.secrets.shown}));
    if let Some(project) = project {
        let trusted = match project.trust {
            Trust::Trusted => json!(true),
            Trust::Distrusted => json!(false),
            Trust::Unknown => Value::Null,
        };
        files.insert(
            "project".to_string(),
            json!({"file": project.file.shown, "trusted": trusted, "version": project.file.version}),
        );
    }
    Value::Object(files)
}

/// 一条问题写成协议上的样子，话照 `words`。现在照什么用着：一项的问题照这一项在它那一层下面几层合出来的；整份的问题
/// 是手里用着的文件 `file` 的，照上一次读好的用（`last_good`，施工 8-4）或者先不用（`nothing`）；查一段字（没有 `file`）
/// 时不说。
pub(super) fn said(
    config: &Config,
    layers: &Layers,
    problem: &Problem,
    file: Option<&File>,
    words: &Human,
) -> Result<Value, Refusal> {
    let place = file.map(|file| (file.shown.as_str(), file.last_good));
    said_at(config, layers, problem, place, words)
}

/// 同 [`said`]，文件只给在哪、是不是照上一次读好的用着（`place`）：密钥文件不是一层配置（施工 8-5）。
pub(crate) fn said_at(
    config: &Config,
    layers: &Layers,
    problem: &Problem,
    place: Option<(&str, bool)>,
    words: &Human,
) -> Result<Value, Refusal> {
    let key = problem.key.as_deref().unwrap_or_default();
    let item = miyu_config::key::item_of(config.items(), key);
    let using = match (problem.code, item) {
        (code, _) if code.whole_file() => place.map(|(_, last_good)| match last_good {
            true => Using::LastGood,
            false => Using::Nothing,
        }),
        (
            Code::WrongType
            | Code::NotAnOption
            | Code::OutOfRange
            | Code::BadFormat
            | Code::WrongLayer
            | Code::NotTightening,
            Some(item),
        ) => below(item, key, layers, problem.layer)
            .map(|(value, origin)| Using::Value(value, origin)),
        _ => None,
    };
    let told = told(problem, config.items(), using.as_ref(), words)?;
    let shown = place.map(|(shown, _)| shown);
    Ok(wire::problem(problem, shown, &told, using.as_ref()))
}

/// 说成话；要用的字缺了是装坏了：内部出错，记一条 `WARN`。
pub(super) fn told(
    problem: &Problem,
    items: &[Item],
    using: Option<&Using>,
    words: &dyn Words,
) -> Result<Told, Refusal> {
    tell(problem, items, using, words).map_err(|missing| {
        tracing::warn!(target: TARGET, error = %missing, "config words missing");
        Refusal::INTERNAL
    })
}

/// 这个连接的语言的字。读不懂是装坏了：内部出错。
pub(crate) fn words(core: &Core, language: &str) -> Result<Human, Refusal> {
    let packages = core.system_packages();
    Human::load(&core.resources, language)
        .map(|human| human.with_packages(crate::packages::manifests(&packages), language))
        .map_err(|error| {
            tracing::warn!(target: TARGET, error = %error, "resource unreadable");
            Refusal::INTERNAL
        })
}

/// 请求里的 `keys` 挑出的几项和真的键，照清单的先后；不写的是全部写死的项（键里有人起的名字的项，`config.schema` 用它的样子）。
/// 一个真的键对得上一项的样子就算（施工 8-6）。有不认识的：`unknown_config_key`。
pub(super) fn selected<'a>(
    config: &'a Config,
    keys: Option<&[String]>,
    words: &Human,
) -> Result<Vec<(&'a Item, String)>, Refusal> {
    let items = config.items();
    let Some(keys) = keys else {
        return Ok(items
            .iter()
            .map(|item| (item, item.key.to_string()))
            .collect());
    };
    let mut unknown = Vec::new();
    let mut found = Vec::new();
    for key in keys {
        match miyu_config::key::item_of(items, key) {
            Some(item) => found.push((item, key.clone())),
            None => {
                let problem = Problem {
                    code: Code::UnknownKey,
                    layer: Layer::System,
                    at: None,
                    key: Some(key.clone()),
                    got: None,
                    shape: None,
                    why: None,
                    suggest: nearest(items.iter().map(|item| item.key), key),
                    current: None,
                    name: None,
                };
                let told = told(&problem, items, None, words)?;
                // 请求里写错的键是这一条请求的错：级别写错误（文件里不认识的键才是警告）。
                let mut entry = wire::problem(&problem, None, &told, None);
                entry["level"] = json!("error");
                unknown.push(entry);
            }
        }
    }
    if !unknown.is_empty() {
        return Err(Refusal::unknown_config_key(unknown));
    }
    found.sort_by_key(|(item, key)| {
        let at = items.iter().position(|listed| listed.key == item.key);
        (at, key.clone())
    });
    found.dedup_by(|a, b| a.1 == b.1);
    Ok(found)
}

/// `config.get` 挑的：写了 `keys` 的照 [`selected`]；没写的是全部写死的项，加上最终值 `resolved` 里键里有人起的名字的
/// 每一个真的键（施工 8-6）。
fn selected_in<'a>(
    config: &'a Config,
    resolved: &miyu_config::merge::Resolved,
    keys: Option<&[String]>,
    words: &Human,
) -> Result<Vec<(&'a Item, String)>, Refusal> {
    if keys.is_some() {
        return selected(config, keys, words);
    }
    let items = config.items();
    let mut found: Vec<(&Item, String)> = items
        .iter()
        .filter(|item| !item.is_pattern())
        .map(|item| (item, item.key.to_string()))
        .collect();
    for key in resolved.keys() {
        if let Some(item) = miyu_config::key::item_of(items, key).filter(|item| item.is_pattern()) {
            found.push((item, key.to_string()));
        }
    }
    Ok(found)
}
