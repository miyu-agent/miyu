//! `miyu acp` 给人看的字（施工 ACP-6，`internal/blueprint/cli/acp.md`「给人看的字」）：模式、配置项、确认的选项的名字，回给编辑器的
//! 出错。编辑器照原样显示给人，不给模型看。

use super::Language;

impl Language {
    /// 模式的名字：`full` 开放权限、`read-only` 只读（同网页、终端界面的说法）。
    pub(crate) fn acp_mode(&self, id: &str) -> &'static str {
        match (self, id) {
            (Language::Chinese, "read-only") => "只读",
            (Language::Chinese, _) => "开放权限",
            (Language::English, "read-only") => "Read-only",
            (Language::English, _) => "Full access",
        }
    }

    /// 会话配置项的名字：`mode` 权限、`model` 模型、`effort` 思考强度。
    pub(crate) fn acp_option(&self, id: &str) -> &'static str {
        match (self, id) {
            (Language::Chinese, "mode") => "权限",
            (Language::Chinese, "model") => "模型",
            (Language::Chinese, _) => "思考强度",
            (Language::English, "mode") => "Permission",
            (Language::English, "model") => "Model",
            (Language::English, _) => "Thinking effort",
        }
    }

    /// 模型配置项里模型池那一组的名字。
    pub(crate) fn acp_pools(&self) -> &'static str {
        match self {
            Language::Chinese => "模型池",
            Language::English => "Model pools",
        }
    }

    /// 确认的选项：`once` 仅允许本次、`session` 在本会话内允许、`deny` 拒绝。
    pub(crate) fn acp_decision(&self, decision: &str) -> &'static str {
        match (self, decision) {
            (Language::Chinese, "once") => "仅允许本次",
            (Language::Chinese, "session") => "在本会话内允许",
            (Language::Chinese, _) => "拒绝",
            (Language::English, "once") => "Allow once",
            (Language::English, "session") => "Allow for this session",
            (Language::English, _) => "Deny",
        }
    }

    /// 一道一道问时不选的那一项。
    pub(crate) fn acp_skip(&self) -> &'static str {
        match self {
            Language::Chinese => "跳过",
            Language::English => "Skip",
        }
    }

    /// 几道题一张表单时上面的那句话。
    pub(crate) fn acp_questions(&self) -> &'static str {
        match self {
            Language::Chinese => "回答下面的问题",
            Language::English => "Answer the questions below",
        }
    }

    /// 表单里给人自己写的那一格的标题：紧跟在那道题的选项后面，不再重复题目。
    pub(crate) fn acp_other(&self) -> &'static str {
        match self {
            Language::Chinese => "其他",
            Language::English => "Other",
        }
    }

    /// 头自己的 `/compact` 在编辑器命令菜单里的说明。
    pub(crate) fn acp_compact(&self) -> &'static str {
        match self {
            Language::Chinese => "压缩上下文",
            Language::English => "Compact the context",
        }
    }

    /// `/compact` 后面可以写什么。
    pub(crate) fn acp_compact_hint(&self) -> &'static str {
        match self {
            Language::Chinese => "摘要要保留的要点",
            Language::English => "what the summary should keep",
        }
    }

    /// 编辑器发来的一行不是 JSON。
    pub(crate) fn acp_unreadable(&self) -> &'static str {
        match self {
            Language::Chinese => "无法解析的消息",
            Language::English => "unreadable message",
        }
    }

    /// 握手以前来了别的请求。
    pub(crate) fn acp_not_initialized(&self) -> &'static str {
        match self {
            Language::Chinese => "需要先发送 initialize",
            Language::English => "initialize first",
        }
    }

    /// 不认识的方法。
    pub(crate) fn acp_no_method(&self, method: &str) -> String {
        match self {
            Language::Chinese => format!("不支持的方法：{method}"),
            Language::English => format!("unsupported method: {method}"),
        }
    }

    /// 工作目录不是绝对路径。
    pub(crate) fn acp_relative_cwd(&self) -> &'static str {
        match self {
            Language::Chinese => "工作目录必须是绝对路径",
            Language::English => "the working directory must be an absolute path",
        }
    }

    /// 没有这个会话，或者它没在这个编辑器里打开。
    pub(crate) fn acp_no_session(&self) -> &'static str {
        match self {
            Language::Chinese => "会话不存在",
            Language::English => "no such session",
        }
    }

    /// 上一句还没有回答完。
    pub(crate) fn acp_busy(&self) -> &'static str {
        match self {
            Language::Chinese => "上一条消息还没有回答完",
            Language::English => "the previous prompt is still running",
        }
    }

    /// 图片、资源的内容解不开。
    pub(crate) fn acp_bad_content(&self) -> &'static str {
        match self {
            Language::Chinese => "图片或文件的内容无法解码",
            Language::English => "cannot decode an image or file",
        }
    }

    /// 既没有字，也没有附件。
    pub(crate) fn acp_empty(&self) -> &'static str {
        match self {
            Language::Chinese => "消息是空的",
            Language::English => "the prompt is empty",
        }
    }

    /// 不认识的模式、配置项或者值。
    pub(crate) fn acp_bad_option(&self) -> &'static str {
        match self {
            Language::Chinese => "不支持的选项",
            Language::English => "unsupported option",
        }
    }

    /// 不是人说话开的一轮在编辑器里的开头：叫醒它的后台任务（标题、`done`、`failed`、`stopped`），不知道的说她自己接着做。
    pub(crate) fn acp_woken(&self, job: Option<(&str, &str)>) -> String {
        match (self, job) {
            (Language::Chinese, Some((title, "done"))) => format!("后台任务「{title}」已完成"),
            (Language::Chinese, Some((title, "failed"))) => format!("后台任务「{title}」失败"),
            (Language::Chinese, Some((title, "stopped"))) => format!("后台任务「{title}」已停止"),
            (Language::Chinese, Some((title, _))) => format!("后台任务「{title}」已结束"),
            (Language::Chinese, None) => "Miyu 自己开始了新的一轮".to_string(),
            (Language::English, Some((title, "done"))) => {
                format!("Background job \"{title}\" finished")
            }
            (Language::English, Some((title, "failed"))) => {
                format!("Background job \"{title}\" failed")
            }
            (Language::English, Some((title, "stopped"))) => {
                format!("Background job \"{title}\" stopped")
            }
            (Language::English, Some((title, _))) => format!("Background job \"{title}\" ended"),
            (Language::English, None) => "Miyu started a new turn on its own".to_string(),
        }
    }

    /// 一轮出错收尾、核心没给说明也没有错误原话时，照怎么结束的说（施工 ACP-11）。
    pub(crate) fn acp_ended(&self, reason: &miyu_kernel::event::EndReason) -> &'static str {
        use miyu_kernel::event::EndReason;
        match (self, reason) {
            (Language::Chinese, EndReason::Restarted) => "核心重启了，这一轮没有做完",
            (Language::Chinese, EndReason::Aborted) => "核心意外退出了，这一轮没有做完",
            (Language::Chinese, EndReason::Error) => "这一轮出错了",
            (Language::Chinese, _) => "这一轮没有做完",
            (Language::English, EndReason::Restarted) => {
                "the core restarted before this turn finished"
            }
            (Language::English, EndReason::Aborted) => {
                "the core exited unexpectedly before this turn finished"
            }
            (Language::English, EndReason::Error) => "the turn failed",
            (Language::English, _) => "the turn did not finish",
        }
    }

    /// 和核心断开了。
    pub(crate) fn acp_disconnected(&self) -> &'static str {
        match self {
            Language::Chinese => "与核心的连接已断开",
            Language::English => "lost connection to the core",
        }
    }
}
