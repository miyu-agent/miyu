//! `remember`（施工 R-3 中）：记一条；写了 `replaces` 的是改那一条。超长的、类不认识的、编号写错的不记，说一句。可以带一行只用来
//! 检索的关键词（施工 R-15 上），太长的截断、不拒。认识的人那一类（`person`）要写是谁（`who`，施工 E-1 上），新记的没写不记、
//! 说一句；改一条没写的带上旧的。

use std::path::Path;

use serde::Deserialize;

use miyu_kernel::tool::Access;
use miyu_recall::{CLASSES, PERSON, keywords_of};
use miyu_tool::load::{self, LoadError};
use miyu_tool::{Call, Done, Progress, REMEMBER, Remember as Remembering, Running, Spec, Tool};

use crate::texts::{Texts, line, memory_id, said};
use crate::{SOFTWARE, TEXT_CHARS};

/// `remember`。
pub(crate) struct Remember {
    spec: Spec,
    texts: Texts,
}

/// 她给的参数。别的参数不认，也不报错。
#[derive(Deserialize)]
struct Args {
    class: String,
    text: String,
    #[serde(default)]
    replaces: Option<String>,
    /// 只用来检索的关键词（施工 R-15 上）：可以不写，太长的截断。
    #[serde(default)]
    keywords: Option<String>,
    /// 说的是谁（施工 E-1 上）：认识的人那一类（`person`）要写，别的类不看。
    #[serde(default)]
    who: Option<String>,
}

impl Remember {
    /// 照资源目录 `resources` 里的说明造，几句是 `texts`。
    pub(crate) fn load(resources: &Path, texts: Texts) -> Result<Remember, LoadError> {
        Ok(Remember {
            spec: load::spec(resources, SOFTWARE, REMEMBER, Access::Read)?,
            texts,
        })
    }
}

impl Tool for Remember {
    fn spec(&self) -> &Spec {
        &self.spec
    }

    fn run(&self, call: Call, _progress: Progress) -> Running<'_> {
        Box::pin(async move {
            let texts = &self.texts;
            let all = texts.all();
            let args = match serde_json::from_str::<Args>(&call.args) {
                Ok(args) => args,
                Err(error) => return texts.bad_args(&error),
            };
            let Some(port) = call.memory.as_ref() else {
                return texts.off();
            };
            if !CLASSES.contains(&args.class.as_str()) {
                return texts.error(
                    &all.unknown_class,
                    "remember/unknown-class",
                    &[("class", &args.class)],
                );
            }
            let who = args
                .who
                .as_deref()
                .map(str::trim)
                .filter(|who| !who.is_empty() && !matches!(*who, "null" | "undefined"))
                .map(str::to_string);
            let person = args.class == PERSON;
            if person && who.is_none() && args.replaces.is_none() {
                return texts.error(&all.no_who, "remember/no-who", &[]);
            }
            let chars = args.text.chars().count();
            if chars > TEXT_CHARS {
                let (chars, limit) = (chars.to_string(), TEXT_CHARS.to_string());
                return texts.error(
                    &all.too_long,
                    "remember/too-long",
                    &[("chars", &chars), ("limit", &limit)],
                );
            }
            // 「没传」的几种写法（空串、`null`）当没传：照基础系统的 `given`。
            let replaces = match args
                .replaces
                .filter(|old| !matches!(old.as_str(), "" | "null" | "undefined"))
            {
                None => None,
                Some(old) => match memory_id(&old) {
                    Ok(old) => Some(old),
                    Err(old) => return texts.no_such(&old),
                },
            };
            let remembering = Remembering {
                class: args.class,
                text: args.text,
                replaces,
                keywords: args.keywords.as_deref().and_then(keywords_of),
                who: who.filter(|_| person),
            };
            match port.save(remembering).await {
                Ok(id) => {
                    let id = id.to_string();
                    match replaces {
                        None => Done::ok(line(&all.saved, &[("id", &id)]))
                            .said(said("remember/saved", &[("id", &id)])),
                        Some(old) => {
                            let fields = [("id", id.as_str()), ("old", &old.to_string())];
                            Done::ok(line(&all.replaced, &fields))
                                .said(said("remember/replaced", &fields))
                        }
                    }
                }
                Err(refused) => texts.refused(&refused),
            }
        })
    }
}
