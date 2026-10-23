//! 预设能关的几块事实（施工 C-11，2026-10-23 项目主人定：环境那一块拆成几个能开关的功能，群聊里用不上的能关掉）：时间（带时区）、
//! 工作目录、权限级别、会话编号、模型各一个开关，都开着的和以前逐字节一样。
//!
//! 环境那一块原来是时间、时区、工作目录一起：只关一样的换成拆开的写法（`core/facts/env-time.txt`、`env-cwd.txt`），类别照旧是
//! `env`，照旧和最近一块比原文；两样都关的不注入。以前造的快照没有拆开的写法，它们的开关也都开着（预设关不了没装的功能），
//! 用不上。

use std::collections::BTreeMap;

use super::{FactTemplates, env_fields, kind};
use crate::event::ContextInjected;
use crate::template::{Template, TemplateError};
use crate::time::Timestamp;

use super::Environment;

/// 哪几块注入：执行器照会话的预设算好交进来（`miyu-policy` 的功能编号）。默认全开。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gates {
    /// 时间和时区。
    pub time: bool,
    /// 工作目录。
    pub workdir: bool,
    /// 权限级别：平常那一份和切了级别的那一份。
    pub permission: bool,
    /// 会话编号。
    pub session: bool,
    /// 模型。
    pub model: bool,
}

impl Default for Gates {
    fn default() -> Gates {
        Gates {
            time: true,
            workdir: true,
            permission: true,
            session: true,
            model: true,
        }
    }
}

/// 开关和环境拆开以后的两份写法（只关时间、只关工作目录时用）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Gated {
    /// 开关。
    pub(super) gates: Gates,
    /// 只有时间、时区的那一份。
    time: Option<Template>,
    /// 只有工作目录的那一份。
    cwd: Option<Template>,
}

impl FactTemplates {
    /// 带上开关和环境拆开以后的两份写法（`time` 只能要 `time`、`timezone`，`cwd` 只能要 `cwd`：关了的那一样写不进去）。
    /// 没有写法的那一样单独开着时不注入环境那一块。
    ///
    /// # Errors
    ///
    /// 写法坏了、要了不该要的字段。
    pub fn with_gates(
        mut self,
        gates: Gates,
        time: Option<&str>,
        cwd: Option<&str>,
    ) -> Result<FactTemplates, TemplateError> {
        let time = time.map(Template::parse).transpose()?;
        if let Some(time) = &time {
            time.render(&BTreeMap::from([("time", ""), ("timezone", "")]))?;
        }
        let cwd = cwd.map(Template::parse).transpose()?;
        if let Some(cwd) = &cwd {
            cwd.render(&BTreeMap::from([("cwd", "")]))?;
        }
        self.gated = Gated { gates, time, cwd };
        Ok(self)
    }

    /// 环境那一块：此刻 `now` 到十分钟（施工 1-13 三补，原来到小时），时区，工作目录。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：造的时候已经拿全部字段试换过；`env` 这个类别名也合写法。
    pub fn env(&self, now: Timestamp, environment: &Environment) -> ContextInjected {
        self.env_with(&self.env, now, environment)
    }

    /// 照开关该注入的环境那一块（施工 C-11）：都开着的是 [`FactTemplates::env`]；只开一样的照拆开的写法；都关了、没有写法的没有。
    pub(super) fn env_due(
        &self,
        now: Timestamp,
        environment: &Environment,
    ) -> Option<ContextInjected> {
        let Gated { gates, time, cwd } = &self.gated;
        let template = match (gates.time, gates.workdir) {
            (true, true) => &self.env,
            (true, false) => time.as_ref()?,
            (false, true) => cwd.as_ref()?,
            (false, false) => return None,
        };
        Some(self.env_with(template, now, environment))
    }

    fn env_with(
        &self,
        template: &Template,
        now: Timestamp,
        environment: &Environment,
    ) -> ContextInjected {
        let time = now.local_ten_minutes(environment.offset);
        let timezone = environment.offset.to_string();
        let fields = env_fields(&time, &timezone, &environment.cwd);
        ContextInjected {
            refs: Vec::new(),
            after: false,
            kind: kind("env"),
            text: template.render(&fields).expect("造的时候试换过，字段都有"),
        }
    }
}
