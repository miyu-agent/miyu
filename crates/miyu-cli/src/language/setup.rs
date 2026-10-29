//! `miyu setup` 给人看的字（施工 8-11，`docs/blueprint/cli/setup.md`「给人看的字」；CL-2 下照设计 47 第三节改：进度、结果行
//! 不带「·」和句号，报错交 [`Problem`]，建议另起一行。选模型的那几句等改选模型的那一步一起改）。key 从不出现在这里的任何
//! 一句里。「未选择」「未收到密钥」「已保存」和贴 key 那一句照 `miyu provider login` 的（`language/login.rs`）。

use super::Language;
use crate::diagnostic::Problem;

impl Language {
    /// 用不了：`after` 的接在「在哪」后面，带开头的逗号。
    pub(crate) fn unusable(&self, why: &str, after: bool) -> String {
        match (self, after) {
            (Language::Chinese, true) => format!("，不可用：{why}"),
            (Language::Chinese, false) => format!("不可用：{why}"),
            (Language::English, true) => format!(", unavailable: {why}"),
            (Language::English, false) => format!("unavailable: {why}"),
        }
    }

    /// 为什么用不了：认不出接口、没有地址、驱动还没有（`driver` 是它的写法）。
    pub(crate) fn unusable_why(&self, driver: Option<&str>, address: bool) -> String {
        match (self, driver, address) {
            (Language::Chinese, None, _) => "未知的接口".to_string(),
            (Language::Chinese, Some(_), false) => "目录中没有地址".to_string(),
            (Language::Chinese, Some(driver), true) => format!("还没有 {driver} 驱动"),
            (Language::English, None, _) => "unknown API".to_string(),
            (Language::English, Some(_), false) => "no address in the catalog".to_string(),
            (Language::English, Some(driver), true) => format!("no {driver} driver yet"),
        }
    }

    /// 问编号。
    pub(crate) fn pick_number(&self) -> &'static str {
        match self {
            Language::Chinese => "选择编号：",
            Language::English => "Pick a number: ",
        }
    }

    /// 敲的不是列出的编号：说完接着问。
    pub(crate) fn not_a_number(&self, typed: &str) -> Problem {
        match self {
            Language::Chinese => Problem::new(format!("无效的编号：{typed}")),
            Language::English => Problem::new(format!("invalid number: {typed}")),
        }
    }

    /// 头看得到、核心看不到的变量：几个名字连起来（2026-10-08 项目主人定的说法，设计 47 改成告警加两条提示）。
    pub(crate) fn core_cannot_see(&self, names: &[&str]) -> Problem {
        match self {
            Language::Chinese => Problem::new(format!(
                "核心未读到 {}（在核心启动后设置）",
                names.join("、")
            ))
            .hint("核心空闲退出后重新运行 'miyu setup'")
            .hint("或选择这一家供应商并粘贴密钥"),
            Language::English => Problem::new(format!(
                "the core has not read {} (set after the core started)",
                names.join(", ")
            ))
            .hint("run 'miyu setup' again after the core exits when idle")
            .hint("or pick that provider and paste the key"),
        }
    }

    /// `--provider` 不是目录里能用的一家。
    pub(crate) fn no_usable_provider(&self, id: &str) -> Problem {
        match self {
            Language::Chinese => Problem::new(format!("目录中没有可用的供应商 '{id}'")),
            Language::English => Problem::new(format!("no usable provider '{id}' in the catalog")),
        }
    }

    /// 不在终端里又没写 `--provider`。
    pub(crate) fn setup_needs_terminal(&self) -> Problem {
        match self {
            Language::Chinese => Problem::new("需要在终端中选择")
                .hint("或者指定供应商：'miyu setup --provider <编号>'"),
            Language::English => Problem::new("picking needs a terminal")
                .hint("or give a provider: 'miyu setup --provider <id>'"),
        }
    }

    /// 试之前。
    pub(crate) fn trying(&self, name: &str) -> String {
        match self {
            Language::Chinese => format!("正在测试 {name}…"),
            Language::English => format!("testing {name}…"),
        }
    }

    /// 通了。
    pub(crate) fn it_works(&self, model: &str, ms: u64) -> String {
        match self {
            Language::Chinese => format!("已连通：测试 {model}，{ms} 毫秒收到第一个字"),
            Language::English => format!("connected: tested {model}, first token in {ms} ms"),
        }
    }

    /// 只列了模型就算通了（施工 8-11 五补）：列出几个。
    pub(crate) fn listed(&self, count: usize) -> String {
        match self {
            Language::Chinese => format!("已连通：列出 {count} 个模型"),
            Language::English => format!("connected: listed {count} models"),
        }
    }

    /// 没通，问仍然保存不保存（施工 8-11 五补）。
    pub(crate) fn save_anyway(&self) -> &'static str {
        match self {
            Language::Chinese => "仍然保存？[y/N] ",
            Language::English => "Save anyway? [y/N] ",
        }
    }

    /// 模型照目录列的。
    pub(crate) fn listed_from_catalog(&self) -> &'static str {
        match self {
            Language::Chinese => "· 供应商列不出模型，下面照 models.dev 的目录列。",
            Language::English => {
                "· The provider listed no models; the list below is from the models.dev catalog."
            }
        }
    }

    /// 不通：哪一步、分类、原话（原话里有 HTTP 状态的照原话，和 `miyu ask` 出错那一行一样）。
    pub(crate) fn did_not_work(&self, stage: &str, class: &str, message: &str) -> Problem {
        let stage = match (self, stage) {
            (Language::Chinese, "config") => "配置",
            (Language::Chinese, "list") => "列模型",
            (Language::Chinese, _) => "发请求",
            (Language::English, "config") => "config",
            (Language::English, "list") => "listing models",
            (Language::English, _) => "request",
        };
        let class = self.class_name(class);
        let mut said = match self {
            Language::Chinese => format!("连接失败（{stage}）：{class}"),
            Language::English => format!("connection failed ({stage}): {class}"),
        };
        let message = message.trim();
        if !message.is_empty() {
            said.push_str(match self {
                Language::Chinese => "：",
                Language::English => ": ",
            });
            said.push_str(message);
        }
        Problem::new(said)
    }

    /// 选模型的头一行。
    pub(crate) fn models_heading(&self) -> &'static str {
        match self {
            Language::Chinese => "选主对话的模型：",
            Language::English => "Pick the model for chat:",
        }
    }

    /// 推荐的，接在模型名后面。
    pub(crate) fn recommended(&self) -> &'static str {
        match self {
            Language::Chinese => "（推荐）",
            Language::English => " (recommended)",
        }
    }

    /// 还有几个没列。
    pub(crate) fn more_models(&self, count: usize) -> String {
        match self {
            Language::Chinese => format!("还有 {count} 个，敲名字也行。"),
            Language::English => format!("{count} more; you can type a name."),
        }
    }

    /// 问模型。
    pub(crate) fn pick_model(&self) -> &'static str {
        match self {
            Language::Chinese => "选一个编号，直接回车用推荐的：",
            Language::English => "Pick a number, or press Enter for the recommended one: ",
        }
    }

    /// 敲的不在列表里。
    pub(crate) fn not_listed(&self, typed: &str) -> String {
        match self {
            Language::Chinese => format!("{typed} 不在列表里"),
            Language::English => format!("{typed} is not in the list"),
        }
    }

    /// 写好了。
    pub(crate) fn set_up(&self, reference: &str) -> String {
        match self {
            Language::Chinese => format!("已设置 models.chat = {reference}"),
            Language::English => format!("set models.chat = {reference}"),
        }
    }

    /// `miyu ask` 没模型，先走 setup。
    pub(crate) fn setup_first(&self) -> &'static str {
        match self {
            Language::Chinese => "未配置模型，开始设置",
            Language::English => "no model configured, starting setup",
        }
    }

    /// 选一家那张表的头一行（施工 8-11 再补）。
    pub(crate) fn pick_provider(&self) -> &'static str {
        match self {
            Language::Chinese => "选择供应商：",
            Language::English => "Pick a provider:",
        }
    }

    /// 环境变量里找到了这一家的 key（施工 8-11 再补）。
    pub(crate) fn key_found(&self) -> &'static str {
        match self {
            Language::Chinese => "已找到密钥",
            Language::English => "key found",
        }
    }

    /// 这一家配置里已经有了（施工 8-11 再补）。
    pub(crate) fn set_up_mark(&self) -> &'static str {
        match self {
            Language::Chinese => "已配置",
            Language::English => "configured",
        }
    }

    /// 本机跑着的服务（施工 8-11 再补）。
    pub(crate) fn local_mark(&self, base_url: &str) -> String {
        match self {
            Language::Chinese => format!("本机 {base_url}"),
            Language::English => format!("local {base_url}"),
        }
    }

    /// 最后一行：自定义（施工 8-11 再补）。
    pub(crate) fn custom(&self) -> &'static str {
        match self {
            Language::Chinese => "自定义",
            Language::English => "Custom",
        }
    }

    /// 问自定义的地址。
    pub(crate) fn base_url(&self) -> &'static str {
        match self {
            Language::Chinese => "Base URL：",
            Language::English => "Base URL: ",
        }
    }

    /// 地址不是 http、https 的：说完接着问。
    pub(crate) fn not_a_url(&self) -> Problem {
        match self {
            Language::Chinese => Problem::new("地址必须以 http:// 或 https:// 开头"),
            Language::English => Problem::new("the address must start with http:// or https://"),
        }
    }

    /// 接口协议那张表的头一行。
    pub(crate) fn protocol_heading(&self) -> &'static str {
        match self {
            Language::Chinese => "接口协议：",
            Language::English => "API protocol:",
        }
    }

    /// 一种接口协议叫什么：驱动的写法 `driver`。
    pub(crate) fn protocol(&self, driver: &str) -> &'static str {
        match (self, driver) {
            (Language::Chinese, "openai-chat") => "OpenAI 兼容",
            (Language::English, "openai-chat") => "OpenAI compatible",
            (_, "anthropic") => "Anthropic",
            _ => "OpenAI Responses",
        }
    }

    /// 问自定义的 key：可以空。
    pub(crate) fn paste_key_optional(&self) -> &'static str {
        match self {
            Language::Chinese => "密钥（不显示，可留空）：",
            Language::English => "Key (hidden, may be empty): ",
        }
    }

    /// 取不到模型列表（2026-10-08 项目主人定的说法）。
    pub(crate) fn no_model_list(&self) -> &'static str {
        match self {
            Language::Chinese => "未获取到模型列表",
            Language::English => "Could not get the model list",
        }
    }

    /// 问模型名。
    pub(crate) fn model_name(&self) -> &'static str {
        match self {
            Language::Chinese => "模型名：",
            Language::English => "Model name: ",
        }
    }

    /// 自定义的一家问 ID，直接回车用自动起的 `suggested`（施工 8-11 六补；8-11 七补「编号」改叫「ID」，2026-10-14 项目主人定）。
    pub(crate) fn custom_id(&self, suggested: &str) -> String {
        match self {
            Language::Chinese => format!("ID（回车使用 {suggested}）："),
            Language::English => format!("ID (Enter for {suggested}): "),
        }
    }

    /// 敲的 ID 不合写法：说完接着问。
    pub(crate) fn bad_custom_id(&self, typed: &str) -> Problem {
        match self {
            Language::Chinese => Problem::new(format!("ID 无效：{typed}"))
                .hint("以小写字母开头，只含小写字母、数字、- 和 _，最多 32 个字符"),
            Language::English => Problem::new(format!("invalid ID: {typed}")).hint(
                "start with a lowercase letter, use only lowercase letters, digits, - and _, at most 32 characters",
            ),
        }
    }

    /// 敲的 ID 配置里已经有了：说完接着问。
    pub(crate) fn custom_id_taken(&self, typed: &str) -> Problem {
        match self {
            Language::Chinese => Problem::new(format!("ID '{typed}' 已存在")),
            Language::English => Problem::new(format!("ID '{typed}' is already taken")),
        }
    }
}
