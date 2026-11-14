//! `end_life`（施工 LF-4 下，设计 `70-推演人生.md` 第二节第 3 条、第三节第 5 条，`docs/blueprint/tools/end_life.md`）：她自己
//! 结束这段人生。包 [`crate::LIFE`] 的清单写了 `needs = ["life"]`：只在没结束的人生会话的工具面上。访问类别是读，同定时、循环：
//! 动的是 Miyu 自己的会话和定时表，不是这台机器。
//!
//! - `last_words` 必写，去掉前后空白不能是空的：没写、空白、参数读不出来的都回同一句，什么都不记。
//! - 经 [`miyu_tool::LifePort`]（定时端口带着）交给端点：端点记下结束就答，后面的清理不等（等了这一轮结束不了）。
//! - 已经结束了的（同一轮里调第二次）回一句，不报错、不再记；不是人生会话的（工具面上本来没有，防万一）回一句。

use std::path::Path;

use serde::Deserialize;

use miyu_kernel::template::Template;
use miyu_kernel::tool::Access;
use miyu_tool::load::{self, LoadError};
use miyu_tool::{Call, Done, END_LIFE, LifeRefused, Progress, Running, Spec, Tool};

use crate::LIFE_SOFTWARE;
use crate::texts::Package;

/// 这个包。
const PACKAGE: Package = Package { id: LIFE_SOFTWARE };

/// `end_life`。
pub(crate) struct EndLife {
    spec: Spec,
    ended: Template,
    required: Template,
    already: Template,
    not_life: Template,
}

/// 她给的参数。
#[derive(Deserialize)]
struct Args {
    #[serde(default)]
    last_words: Option<String>,
}

impl EndLife {
    /// 照资源目录 `resources` 里的字造。
    pub(crate) fn load(resources: &Path) -> Result<EndLife, LoadError> {
        let end = |name: &str| PACKAGE.text(resources, ("end", name), &[]);
        Ok(EndLife {
            spec: load::spec(resources, LIFE_SOFTWARE, END_LIFE, Access::Read)?,
            ended: end("ended")?,
            required: end("required")?,
            already: end("already")?,
            not_life: end("not-life")?,
        })
    }

    /// 端口拒了，或者这次调用没有端口。
    fn refused(&self, refused: LifeRefused) -> Done {
        match refused {
            LifeRefused::Ended => PACKAGE.ok(&self.already, "end/already", &[]),
            // 用不了的（核心在停、会话停了）：这一轮多半也记不下了，照不是人生会话说，端点另记一行。
            LifeRefused::NotLife | LifeRefused::Unavailable => {
                PACKAGE.error(&self.not_life, "end/not-life", &[])
            }
        }
    }
}

impl Tool for EndLife {
    fn spec(&self) -> &Spec {
        &self.spec
    }

    fn run(&self, call: Call, _progress: Progress) -> Running<'_> {
        Box::pin(async move {
            let words = serde_json::from_str::<Args>(&call.args)
                .ok()
                .and_then(|args| args.last_words)
                .filter(|words| !words.trim().is_empty());
            let Some(words) = words else {
                return PACKAGE.error(&self.required, "end/required", &[]);
            };
            let port = call.schedules.as_ref().and_then(|port| port.life());
            let (Some(port), Some(ids)) = (port, call.ids.as_ref()) else {
                return self.refused(LifeRefused::NotLife);
            };
            match port.end(&ids.session, &ids.call, words).await {
                Ok(()) => PACKAGE.ok(&self.ended, "end/ended", &[]),
                Err(refused) => self.refused(refused),
            }
        })
    }
}
