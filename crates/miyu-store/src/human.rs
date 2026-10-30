//! 给人看的字（`docs/designs/26-提示词.md` 第三节「写法」里的「双槽」、第八节，施工 4-5 上）。
//!
//! 内核和每个软件包各有一份 `human/<语言>.json`：内核的在 `core/human/`，软件包的在 `software/<软件包>/human/`。
//! 每份里三样：
//!
//! - `tools`：每件工具给人看的显示名 `name`，显示名后面跟哪一个参数的值 `subject`，写在最前面的符号 `icon`，标题
//!   下面还印一块什么 `block`（施工 4-11）；
//! - `said`：每一种说法的字，模板照 `{字段}` 写，编号照这一份所在的地方往下写，例如内核那一份里的
//!   `tool-results/unattended` 就是说法 `core/tool-results/unattended`；
//! - `config`：配置项的名字、说明、选项名，设置页的页和组的名字（[`ConfigWords`]，施工 8-1）。读好的字照
//!   [`Words`] 交给配置清单生成 JSON Schema 和参考文件，那几句话是内核那一份 `said` 里的 `config/…`。
//!
//! 头照一次调用的说法（`tool.result` 的 `human`）换成字。这些字不进请求，所以换进去的字段不转义成 JSON 的样子，
//! 只把控制字符换成 `�`（[`clean`]）：路径、参数是她给的，里面要是混着终端的控制序列，原样印出来会把终端弄乱。
//!
//! 哪一份没有这种语言，照英文那一份；英文也没有的，那一处就没有给人看的字，头照工具名、状态写最泛的。
//!
//! `human.get`（施工 W-1）把这份读好的字整个交给头：工具的样子（[`Human::tools`]）、说法的模板原文、一个字不换
//! （[`Human::said_entries`]）。模板只留解好的 [`Template`] 不够，所以每一句说法这里多存一份原文（内部的 `Phrase`）。
//!
//! 说法管着的数是 1 时，编号多接一段 `/one`（施工 4-5 再补「一个的时候说单数」）：发说法的那一处自己挑，是 1 就
//! 发 `X/one`，别的数照旧发 `X`。英文的 `said` 要是这一句需要单数（`{count}`、`{total}` 后面紧跟着名词），就自己
//! 写上 `X/one` 那一句；中文、日文不挑单复数，不用写，[`Human::load`] 读完一种语言以后，凡是有 `X` 没有 `X/one`
//! 的，拿 `X` 的内容照抄一份补上。

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use miyu_config::package::settings::OFF;
use miyu_config::package::{Choice, Manifest, Setting, SettingKind};
use miyu_config::phrases::Phrases;
use miyu_config::{ConfigWords, ItemWords, Words};
use miyu_kernel::event::Said;
use miyu_kernel::template::Template;

use crate::resources::ResourceRoot;

mod shelf;

pub use shelf::Shelf;

/// 找不到别的语言时用的那一种。
pub const FALLBACK: &str = "en";

/// 读好的一种语言的字。
#[derive(Debug, Clone, Default)]
pub struct Human {
    tools: BTreeMap<String, Face>,
    /// 每一句共用：拷一份 [`Human`]（放着的那份交出去时、补单数时）不重新解模板（施工 V-30）。
    said: BTreeMap<String, Arc<Phrase>>,
    config: ConfigWords,
}

/// 一句说法：原文和解好的模板。`human.get` 要交出原文，一个字不换（施工 W-1）。
#[derive(Debug, Clone)]
struct Phrase {
    /// 原文：`human/<语言>.json` 里 `said` 那一格写的那一句。
    source: String,
    /// 解好的模板：换字段用（[`Human::say`]）。
    template: Template,
}

/// 一件工具给人看的样子。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Face {
    /// 显示名，例如「读取」。
    pub name: String,
    /// 显示名后面跟哪一个参数的值，例如 `file_path`。没有的只写显示名。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    /// 写在最前面的符号，例如 `→`（施工 4-11）。没有的由头定。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// 标题下面还印一块什么（施工 4-11）。没有的只印标题。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block: Option<Block>,
}

/// 标题下面的那一块。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Block {
    /// 执行命令：标题写成 `$ 命令`，不写显示名；下面印工具自己写的结果。
    Command,
    /// 改动：下面印参数 `edits` 里每一处改掉的、改成的。
    Edits,
}

/// `human/<语言>.json` 的样子。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    #[serde(default)]
    tools: BTreeMap<String, Face>,
    #[serde(default)]
    said: BTreeMap<String, String>,
    #[serde(default)]
    config: ConfigWords,
}

/// 一份给人看的字读不懂。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HumanError {
    /// 哪一份。
    pub file: PathBuf,
    /// 为什么。
    pub why: String,
}

impl fmt::Display for HumanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.file.display(), self.why)
    }
}

impl std::error::Error for HumanError {}

impl Human {
    /// 照资源目录 `root` 读 `language` 这一种：内核的一份，每个软件包各一份。
    ///
    /// # Errors
    ///
    /// 有一份读得到、却读不懂（不是 JSON、写法不对、模板坏了），说是哪一份。读不到的不算错：没有这种语言的照
    /// 英文，英文也没有的，那一处就没有字。
    ///
    /// 读好的放在资源目录的架子上（[`Shelf`]，施工 V-30）：再要同一种语言的，先看读过的那几份文件（连同当时没有的）
    /// 长短、修改时刻变没变，没变的交回放着的那份的拷贝，不再读、不再解模板；变了的照旧重读（改了资源，核心不重启下一次
    /// 就是新的）。[`ResourceRoot::forget_words`] 以后再读。
    pub fn load(root: &ResourceRoot, language: &str) -> Result<Human, HumanError> {
        let places = places(root);
        let held = root
            .shelf
            .get(language, &places, || Human::read(&places, language))?;
        Ok(Human::clone(&held))
    }

    /// 照 `places` 的先后读 `language` 这一种，不看架子。
    fn read(places: &[(PathBuf, String)], language: &str) -> Result<Human, HumanError> {
        let mut human = Human::default();
        for (dir, prefix) in places {
            human.add(dir, prefix, language)?;
        }
        human.fill_singular();
        Ok(human)
    }

    /// 再从目录 `dir` 补读一份 `language`（`dir/human/<语言>.json`），说法的编号前面加上 `prefix`（施工 S-6 上）：自己带着
    /// 字的包不在资源目录的 `software/` 下，例如接入QQ 的字在它包文件夹的 `data/` 里，编号照旧是 `software/onebot/…`。读法同
    /// [`Human::load`] 里的每一份：没有这种语言的照英文，英文也没有的不算错；补读完再补一遍单数（`X/one`）。
    ///
    /// # Errors
    ///
    /// 那一份读得到、却读不懂（不是 JSON、写法不对、模板坏了），说是哪一份。
    pub fn with_dir(
        mut self,
        dir: &Path,
        prefix: &str,
        language: &str,
    ) -> Result<Human, HumanError> {
        self.add(dir, prefix, language)?;
        self.fill_singular();
        Ok(self)
    }

    /// 凡是有 `X` 没有 `X/one` 的，拿 `X` 补上 `X/one`（施工 4-5 再补「一个的时候说单数」）：英文那份自己写了
    /// 需要的那些 `X/one`，照它；中文、日文没写，退到这里，补出来的字跟 `X` 一个字不差（这两种语言不挑单复数）。
    fn fill_singular(&mut self) {
        let missing: Vec<(String, Arc<Phrase>)> = self
            .said
            .iter()
            .filter(|(key, _)| !key.ends_with("/one"))
            .filter_map(|(key, phrase)| {
                let one = format!("{key}/one");
                (!self.said.contains_key(&one)).then(|| (one, Arc::clone(phrase)))
            })
            .collect();
        self.said.extend(missing);
    }

    /// 读 `dir` 下 `human/` 里 `language` 那一份，没有就读英文那一份；说法的编号前面加上 `prefix`。
    fn add(&mut self, dir: &Path, prefix: &str, language: &str) -> Result<(), HumanError> {
        let found = [language, FALLBACK].iter().find_map(|language| {
            let file = dir.join("human").join(format!("{language}.json"));
            std::fs::read_to_string(&file).ok().map(|text| (file, text))
        });
        let Some((file, text)) = found else {
            return Ok(());
        };
        let bad = |why: String| HumanError {
            file: file.clone(),
            why,
        };
        let parsed: File = serde_json::from_str(&text).map_err(|error| bad(error.to_string()))?;
        for (key, source) in parsed.said {
            let template =
                Template::parse(&source).map_err(|error| bad(format!("{key}: {error}")))?;
            self.said.insert(
                format!("{prefix}/{key}"),
                Arc::new(Phrase { source, template }),
            );
        }
        self.tools.extend(parsed.tools);
        self.config.items.extend(parsed.config.items);
        self.config.pages.extend(parsed.config.pages);
        self.config.groups.extend(parsed.config.groups);
        Ok(())
    }

    /// 叫 `name` 的工具给人看的样子；没有的是空的。
    pub fn tool(&self, name: &str) -> Option<&Face> {
        self.tools.get(name)
    }

    /// 每件工具给人看的样子，照工具名排好（`human.get`，施工 W-1）。
    pub fn tools(&self) -> &BTreeMap<String, Face> {
        &self.tools
    }

    /// 每一句说法的原文，照编号排好，一个字不换（`human.get`，施工 W-1：换字段是头的事）。
    pub fn said_entries(&self) -> impl Iterator<Item = (&str, &str)> {
        self.said
            .iter()
            .map(|(key, phrase)| (key.as_str(), phrase.source.as_str()))
    }

    /// 照说法换成一句话，字段先过一遍 [`clean`]。没有这一句、或者少了字段的，是空的。
    pub fn say(&self, said: &Said) -> Option<String> {
        let phrase = self.said.get(&said.key)?;
        let fields: BTreeMap<&str, &str> = said
            .fields
            .iter()
            .map(|(field, value)| (field.as_str(), value.as_str()))
            .collect();
        phrase.template.fill(&fields, clean).ok()
    }

    /// 并进软件包的配置项的字（施工 9-1 下，`packages.md`「配置项」）：每个包一组，组名是包的名字；每一项的名字、说明用
    /// 清单里的。照语言 `language` 挑，这种语言没写的照 `en`、`zh`、`ja` 的先后，都没写的名字是键、说明是空的。
    #[must_use]
    pub fn with_packages<'a>(
        mut self,
        packages: impl IntoIterator<Item = (&'a str, &'a Manifest)>,
        language: &str,
    ) -> Human {
        let pick = |phrases: &Phrases| {
            [language, "en", "zh", "ja"]
                .iter()
                .find_map(|language| phrases.get(*language).cloned())
        };
        // 每个人关掉某个包的那一项（施工 U23 末上）：每个不是必需的包一份，字照带占位的那一项。
        let off = self.config.items.get(OFF).cloned();
        for (id, manifest) in packages {
            if let Some(name) = pick(&manifest.name) {
                self.config.groups.insert(id.to_string(), name);
            }
            if let Some(words) = off.as_ref() {
                self.config
                    .items
                    .insert(miyu_config::key::fill(OFF, &[id]), words.clone());
            }
            for setting in &manifest.settings {
                let key = format!("{}.{}", manifest.prefix_of(id), setting.name);
                let words = setting_words(setting, &key, &pick);
                self.config.items.insert(key, words);
            }
            // 扩展的搜索来源（施工 N-5）：下拉里的名字照清单。
            if let Some(words) = self
                .config
                .items
                .get_mut(miyu_config::package::SEARCH_SOURCE)
            {
                for source in &manifest.search {
                    let name = pick(&source.name).unwrap_or_else(|| source.option(id));
                    words.options.insert(source.option(id), name);
                }
            }
        }
        self
    }

    /// 设置页编号 `id` 那一页的名字；没有的是空的（施工 8-2，`config.schema`）。
    pub fn page(&self, id: &str) -> Option<&str> {
        self.config.pages.get(id).map(String::as_str)
    }

    /// 设置页编号 `id` 那一组的名字；没有的是空的（施工 8-2，`config.schema`）。
    pub fn group(&self, id: &str) -> Option<&str> {
        self.config.groups.get(id).map(String::as_str)
    }

    /// 说法 `key` 这一句要哪些字段；没有这一句的是空的。
    pub fn fields(&self, key: &str) -> Option<Vec<&str>> {
        self.said.get(key).map(|phrase| phrase.template.fields())
    }
}

/// 配置清单要的字：项照 `config` 那一格，几句话照内核那一份 `said` 里的，编号前面加上 `core/`。
impl Words for Human {
    fn item(&self, key: &str) -> Option<&ItemWords> {
        self.config.items.get(key)
    }

    fn sentence(&self, key: &str, fields: &[(&str, &str)]) -> Option<String> {
        let said = fields
            .iter()
            .fold(Said::new(format!("core/{key}")), |said, (field, value)| {
                said.with(field, *value)
            });
        self.say(&said)
    }
}

/// 资源目录里有给人看的字的几处、说法编号的前缀，照读的先后：内核的，再照名字的先后每个出厂包（链接不算）。`software/`
/// 读不了的当没有包。
fn places(root: &ResourceRoot) -> Vec<(PathBuf, String)> {
    let software = root.path().join("software");
    let mut packages: Vec<String> = match std::fs::read_dir(&software) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect(),
        Err(_) => Vec::new(),
    };
    packages.sort();
    let mut places = vec![(root.path().join("core"), "core".to_string())];
    places.extend(
        packages
            .into_iter()
            .map(|package| (software.join(&package), format!("software/{package}"))),
    );
    places
}

/// 给人看的字段：控制字符换成 `�`，别的照原样。
pub fn clean(value: &str) -> String {
    value
        .chars()
        .map(|c| if c.is_control() { '\u{FFFD}' } else { c })
        .collect()
}

/// 包清单里一项的字（施工 9-1 下）：名字、说明照语言挑；选项的名字、说明（施工 PC-1，没写名字的不放，`config.schema` 照值写）；
/// 一组字段的每个字段照同样挑（施工 PC-2）。没写名字的项照键 `key`。
fn setting_words(
    setting: &Setting,
    key: &str,
    pick: &dyn Fn(&Phrases) -> Option<String>,
) -> ItemWords {
    let element = match &setting.kind {
        SettingKind::List(element) => element.as_ref(),
        kind => kind,
    };
    let choices = match element {
        SettingKind::Option(choices) => choices.as_slice(),
        _ => &[],
    };
    let said = |words: fn(&Choice) -> &Phrases| -> BTreeMap<String, String> {
        choices
            .iter()
            .filter_map(|choice| Some((choice.value.clone(), pick(words(choice))?)))
            .collect()
    };
    let fields = match element {
        SettingKind::Record { fields, .. } => fields
            .iter()
            .map(|field| (field.name.clone(), setting_words(field, &field.name, pick)))
            .collect(),
        _ => BTreeMap::new(),
    };
    ItemWords {
        name: pick(&setting.label).unwrap_or_else(|| key.to_string()),
        description: pick(&setting.description).unwrap_or_default(),
        options: said(|choice| &choice.name),
        option_descriptions: said(|choice| &choice.description),
        fields,
        suggestions: Vec::new(),
    }
}
