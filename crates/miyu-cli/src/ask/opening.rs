//! 新开的会话怎么开（施工 AP-2，设计 50 第三节）：人格、预设、开局的权限级别；不写的照配置。接着说的会话开局时就定了：
//! 和 `-c`、`-s` 一起写的读参数时就拒（照「参数写错时」，退出码 2）。人格、预设跟着 `session.create` 走；权限级别造会话
//! 不收，开好以后、订阅之前另发 `session.set_permission_level`：放宽的下一次请求生效，这一句开的第一轮就用上。

use clap::ValueEnum;
use serde_json::{Value, json};

use super::{Plan, Screen};
use crate::link;
use crate::rpc::Rpc;

/// 新开的会话怎么开：都不写的照配置。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Opening {
    /// 用哪个人格（`--persona`、`--no-persona`）。
    pub persona: PersonaChoice,
    /// 用哪个预设（`--preset`）：编号原样交给核心，没有的由核心说。
    pub preset: Option<String>,
    /// 开局的权限级别（`--permission`）。
    pub permission: Option<Permission>,
}

/// 用哪个人格。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum PersonaChoice {
    /// 照配置：`session.create` 不写 `persona`。
    #[default]
    Configured,
    /// 用这一个：编号原样交给核心，没有的由核心说。
    Named(String),
    /// 不用人格：`persona` 写 `null`。
    Without,
}

/// 开局的权限级别：命令行上的写法是 `read-only`、`workspace`、`full`。协议上是常用的那一级 `level`（`workspace`、`full`）加
/// 只读开关 `read_only`：只读开着的时候实际的级别就是只读。
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Permission {
    /// 只读：只开只读开关，常用的那一级照配置。
    ReadOnly,
    /// 工作区：执行命令在沙盒里。只读开关关上。
    Workspace,
    /// 完全放开：不启用沙盒。只读开关关上。
    Full,
}

impl Permission {
    /// `session.set_permission_level` 的参数。
    fn params(self, session: &str) -> Value {
        match self {
            Permission::ReadOnly => json!({"session": session, "read_only": true}),
            Permission::Workspace => {
                json!({"session": session, "level": "workspace", "read_only": false})
            }
            Permission::Full => json!({"session": session, "level": "full", "read_only": false}),
        }
    }
}

impl Opening {
    /// 写进 `session.create` 的参数 `params`：人格、预设，没写的不写。
    pub(crate) fn fill(&self, params: &mut Value) {
        match &self.persona {
            PersonaChoice::Configured => {}
            PersonaChoice::Named(persona) => params["persona"] = json!(persona),
            PersonaChoice::Without => params["persona"] = Value::Null,
        }
        if let Some(preset) = &self.preset {
            params["preset"] = json!(preset);
        }
    }
}

/// 写了 `--permission` 的：新开的会话 `session` 开好以后切到那一级。没写的什么都不做；被拒的照核心说的原因说，交回退出码。
pub(crate) async fn set_permission(
    rpc: &mut Rpc,
    session: &str,
    plan: &Plan,
    screen: &mut Screen<'_>,
) -> Result<(), u8> {
    let Some(permission) = plan.opening.permission else {
        return Ok(());
    };
    link::request(
        rpc,
        "session.set_permission_level",
        permission.params(session),
        &plan.language,
        screen.err,
    )
    .await
    .map(|_| ())
}
