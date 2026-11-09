//! `advisor`（`docs/blueprint/tools/advisor.md`，设计 `69-旗舰顾问.md`，施工 ADV-2）：她卡住了，把问题交给旗舰池里的模型，同步
//! 等它答完，回答原文当工具结果交回。只声明 `question`（必写）、`continue`（布尔）；经 [`Call::agents`] 交给执行器
//! （[`miyu_tool::AgentPort::advise`]），顾问会话怎么造、找哪一个接着说是执行器、会话表的事。
//!
//! 工具一直在工具面上，说明不随旗舰池有没有模型变（设计 69 第二节：工具面会话内字节恒定）；池里没有可用模型的，端口不发请求，
//! 这里交回 `advisor/no-model.txt` 那一句。

use std::path::Path;

use serde::Deserialize;
use serde_json::json;

use miyu_kernel::template::Template;
use miyu_kernel::tool::Access;
use miyu_tool::{ADVISOR, Call, Done, NotAdvised, Progress, Running, Spec, Tool};

use crate::common::{Common, said};
use crate::load::{self, LoadError, say};

/// `advisor`。
pub(crate) struct Advisor {
    spec: Spec,
    texts: Texts,
}

/// 输出里给她看的几句：`software/basesystem/advisor/*.txt`，和几件工具共用的。
#[derive(Clone)]
struct Texts {
    common: Common,
    /// 旗舰池里没有可用模型（`advisor/no-model.txt`）。
    no_model: Template,
    /// 顾问没答成，换进原因（`advisor/failed.txt`）。
    failed: Template,
}

/// 她给的参数。别的参数不认，也不报错（同 `subagent`）。
#[derive(Deserialize)]
struct Args {
    question: String,
    /// 接着上一次的顾问会话说：不写、写 `null`、`false` 的新开一个。
    #[serde(default, rename = "continue")]
    continued: Option<bool>,
}

impl Advisor {
    /// 照资源目录 `resources` 里的字造，共用的几句是 `common`。访问类别是读：问一句什么都不改，顾问没有工具。
    pub(crate) fn load(resources: &Path, common: Common) -> Result<Advisor, LoadError> {
        let text = |name: &str, fields: &[&str]| load::text(resources, ADVISOR, name, fields);
        Ok(Advisor {
            spec: load::spec(resources, ADVISOR, Access::Read)?,
            texts: Texts {
                common,
                no_model: text("no-model", &[])?,
                failed: text("failed", &["reason"])?,
            },
        })
    }
}

impl Tool for Advisor {
    fn spec(&self) -> &Spec {
        &self.spec
    }

    fn run(&self, call: Call, _progress: Progress) -> Running<'_> {
        let texts = self.texts.clone();
        Box::pin(async move {
            let args = match serde_json::from_str::<Args>(&call.args) {
                Ok(args) => args,
                Err(error) => return texts.common.bad_args(&error),
            };
            let failed = |why: &NotAdvised| {
                let reason = why.to_string();
                Done::error(say(&texts.failed, &[("reason", &reason)]))
                    .said(said("advisor/failed").with("reason", reason))
            };
            // 没有端口（测试里的假调用、脚本里调的）：照问不了说。
            let Some(port) = call.agents.clone() else {
                return failed(&NotAdvised::Failed("no advisor here".to_string()));
            };
            let continued = args.continued.unwrap_or(false);
            match port.advise(&args.question, continued).await {
                // 回答原文，末尾不加东西（设计 69 第三节第 3 条）；顾问会话的编号给头点进去看（`detail`，施工 ADV-3）。
                Ok(advised) => {
                    let session = advised.session.to_string();
                    Done::ok(advised.answer)
                        .said(said("advisor/answered").with("session", &session))
                        .detail(json!({ "session": session }))
                }
                Err(NotAdvised::NoModel) => {
                    Done::error(say(&texts.no_model, &[])).said(said("advisor/no-model"))
                }
                Err(why) => failed(&why),
            }
        })
    }
}
