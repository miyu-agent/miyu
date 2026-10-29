//! 清单读不成时的代码（施工 9-1 上；施工 F-1 从 `package.rs` 挪出来：那边放不下了）。

/// 问题的代码：给人看的那一句照它在 `core/human/<语言>.json` 的 `package-problems/<code>` 找。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Code {
    /// 读不成 TOML。
    Syntax,
    /// 不认识的表。
    UnknownTable,
    /// 该是表的不是表。
    NotATable,
    /// 表里不认识的键。
    UnknownKey,
    /// 少了必写的：`[package]` 或者某一格。
    MissingKey,
    /// 这张表、这一格不给这种包（`kinds::tables`；`required` 只给内置包）。
    WrongKind,
    /// `[process]`、`[check]` 要有 `[command]`。
    NeedsCommand,
    /// 程序表写了不止一张（施工 F-8 上补，设计 31 第二节第 2 条）：`[ui]`、`[process]`、`[builtin]`、`[worker]` 至多一张。
    TwoPrograms,
    /// `protocol` 不是两个非负整数、最低不大于最高。
    BadProtocol,
    /// 该是字的不是字。
    NotText,
    /// 该是字的数组的不是。
    NotTexts,
    /// 该是「语言到一句话」的不是表。
    NotPhrases,
    /// 不认识的语言。
    UnknownLanguage,
    /// 某种语言那一句空了、不是字。
    EmptyPhrase,
    /// 子命令名的写法不对。
    BadCommandName,
    /// 程序名带了路径、是空的。
    BadProgram,
    /// `start` 不是 `manual`、`always`。
    BadStart,
    /// `opens` 里的页名写法不对。
    BadPage,
    /// `pages_dir` 不是包文件夹里的相对目录（施工 G-15 起相对包文件夹）。
    BadPagesDir,
    /// 两层里同一个编号：数据根那一份（`miyu-store` 认）。
    Duplicate,
    /// 子命令名被先读到的包占了（`miyu-store` 认）。
    CommandTaken,
    /// 配置项的名字写法不对。
    BadSettingName,
    /// 配置项的 `type` 不认识。
    BadType,
    /// 列表的 `element` 不认识、是列表、不是字（施工 9-1 补）。
    BadElement,
    /// 默认值不合类型、不在选项里，密钥写了默认值。
    BadDefault,
    /// 选项少于两个、有重复、不是字。
    BadChoices,
    /// `min`、`max` 不是整数、最小大于最大。
    BadRange,
    /// `layers` 不是只写着 `system`。
    BadLayers,
    /// `applies` 不认识。
    BadApplies,
    /// `hidden` 不是开关。
    NotBool,
    /// 包的编号和核心自己的模块撞了：它的配置项一项都不收（核心起来时、`miyu check` 认）。
    SettingsTaken,
    /// `[process] capabilities` 里有不认识的、重复的名字（施工 9-4 下上）。
    BadCapability,
    /// 功能的编号写法不对（施工 F-1）。
    BadFeature,
    /// 功能下的工具名写法不对、同一个包里列了两次（施工 F-1）。
    BadTool,
    /// `[connection] platform` 写法不对（施工 F-1）。
    BadPlatform,
    /// `[depends]`、`[recommends]` 里有写法不对、重复的包编号（施工 F-1）。
    BadDependency,
    /// 功能的编号被先读到的包占了（施工 F-1，`miyu-store` 认）。
    FeatureTaken,
    /// 清单是内置包，核心里没编进它的代码（施工 F-2，核心起来时认）。
    NotBuiltIn,
    /// `[package] icon` 不是 Lucide 图标名的写法（施工 F-6 上）。
    BadIcon,
    /// `[package] logo` 不是包目录里一个 `.png`、`.webp` 的相对路径（施工 F-18 上）。
    BadLogo,
    /// `[page] dir` 不是包目录里的相对目录（施工 F-6 上）。
    BadPageDir,
    /// `[mascot] model` 不是包目录里的相对路径（施工 F-7）。
    BadMascotModel,
    /// `[theme] file` 不是包目录里的相对路径（施工 F-16）。
    BadThemeFile,
    /// `[export] methods` 里有不是方法名的、重复的（施工 X-18）。
    BadExport,
    /// `[[mcp]]` 写错了（施工 X-2）：服务名、命令、环境变量、给哪种会话、时限，同一个包里服务名重了。
    BadMcp,
    /// `[[slash]]` 写错了（施工 X-14 下）：命令名、说明、提示、给哪种会话，同一个包里重了，没声明 `commands` 能力。
    BadSlash,
    /// `[[search]]` 写错了（施工 N-5）：编号、名字，同一个包里重了，没声明 `search` 能力。
    BadSearch,
    /// `[[side]]` 写错了（施工 Q-3）：编号、档位、名字、说明，同一个包里重了。
    BadSide,
    /// `[package] author` 不是一行 1 到 100 个字（施工 F-12）。
    BadAuthor,
    /// `[package] upstream` 不是 `https://` 开头的网址（施工 F-12）。
    BadUpstream,
    /// `[[widget]]` 写错了（施工 X-15 上）：编号、位置、标题、范围、命令名，同一个包里重了、太多，没声明 `widgets` 能力。
    BadWidget,
    /// `[content.<包>]` 写错了（施工 X-22）：不是包的编号、不是表、`dir` 不是包目录里的相对目录。
    BadContent,
    /// `[package] needs` 里有不认识的、重复的词（施工 X-25）。
    BadNeeds,
    /// `[package] groups` 里的组名写法不对、重复，或者不在出厂的组表里（施工 BS-1 上中）。
    BadGroup,
    /// `[package] targets` 里有不认识的、重复的系统（施工 G-1）。
    BadTargets,
    /// `[package] miyu` 不是三段数字的版本（施工 X-31）。
    BadMiyu,
    /// 要比这一版新的 Miyu（施工 X-31）：`detail` 是清单写的版本。读清单时先比它，比不过的不再往下读。
    NeedsNewerMiyu,
}

impl Code {
    /// 协议、给人看的字里的写法。
    pub fn as_str(self) -> &'static str {
        match self {
            Code::Syntax => "syntax",
            Code::UnknownTable => "unknown_table",
            Code::NotATable => "not_a_table",
            Code::UnknownKey => "unknown_key",
            Code::MissingKey => "missing_key",
            Code::WrongKind => "wrong_kind",
            Code::NeedsCommand => "needs_command",
            Code::TwoPrograms => "two_programs",
            Code::BadProtocol => "bad_protocol",
            Code::NotText => "not_text",
            Code::NotTexts => "not_texts",
            Code::NotPhrases => "not_phrases",
            Code::UnknownLanguage => "unknown_language",
            Code::EmptyPhrase => "empty_phrase",
            Code::BadCommandName => "bad_command_name",
            Code::BadProgram => "bad_program",
            Code::BadStart => "bad_start",
            Code::BadPage => "bad_page",
            Code::BadPagesDir => "bad_pages_dir",
            Code::Duplicate => "duplicate",
            Code::CommandTaken => "command_taken",
            Code::BadSettingName => "bad_setting_name",
            Code::BadType => "bad_type",
            Code::BadElement => "bad_element",
            Code::BadDefault => "bad_default",
            Code::BadChoices => "bad_choices",
            Code::BadRange => "bad_range",
            Code::BadLayers => "bad_layers",
            Code::BadApplies => "bad_applies",
            Code::NotBool => "not_bool",
            Code::SettingsTaken => "settings_taken",
            Code::BadCapability => "bad_capability",
            Code::BadFeature => "bad_feature",
            Code::BadTool => "bad_tool",
            Code::BadPlatform => "bad_platform",
            Code::BadDependency => "bad_dependency",
            Code::FeatureTaken => "feature_taken",
            Code::NotBuiltIn => "not_built_in",
            Code::BadIcon => "bad_icon",
            Code::BadLogo => "bad_logo",
            Code::BadPageDir => "bad_page_dir",
            Code::BadMascotModel => "bad_mascot_model",
            Code::BadThemeFile => "bad_theme_file",
            Code::BadExport => "bad_export",
            Code::BadMcp => "bad_mcp",
            Code::BadSlash => "bad_slash",
            Code::BadSearch => "bad_search",
            Code::BadSide => "bad_side",
            Code::BadAuthor => "bad_author",
            Code::BadUpstream => "bad_upstream",
            Code::BadWidget => "bad_widget",
            Code::BadContent => "bad_content",
            Code::BadNeeds => "bad_needs",
            Code::BadGroup => "bad_group",
            Code::BadTargets => "bad_targets",
            Code::BadMiyu => "bad_miyu",
            Code::NeedsNewerMiyu => "needs_newer_miyu",
        }
    }

    /// 全部代码：给人看的字的门禁照它查三种语言都有。
    pub const ALL: [Code; 55] = [
        Code::Syntax,
        Code::UnknownTable,
        Code::NotATable,
        Code::UnknownKey,
        Code::MissingKey,
        Code::WrongKind,
        Code::NeedsCommand,
        Code::TwoPrograms,
        Code::BadProtocol,
        Code::NotText,
        Code::NotTexts,
        Code::NotPhrases,
        Code::UnknownLanguage,
        Code::EmptyPhrase,
        Code::BadCommandName,
        Code::BadProgram,
        Code::BadStart,
        Code::BadPage,
        Code::BadPagesDir,
        Code::Duplicate,
        Code::CommandTaken,
        Code::BadSettingName,
        Code::BadType,
        Code::BadElement,
        Code::BadDefault,
        Code::BadChoices,
        Code::BadRange,
        Code::BadLayers,
        Code::BadApplies,
        Code::NotBool,
        Code::SettingsTaken,
        Code::BadCapability,
        Code::BadFeature,
        Code::BadTool,
        Code::BadPlatform,
        Code::BadDependency,
        Code::FeatureTaken,
        Code::NotBuiltIn,
        Code::BadIcon,
        Code::BadLogo,
        Code::BadPageDir,
        Code::BadMascotModel,
        Code::BadThemeFile,
        Code::BadExport,
        Code::BadMcp,
        Code::BadSlash,
        Code::BadSearch,
        Code::BadSide,
        Code::BadAuthor,
        Code::BadUpstream,
        Code::BadWidget,
        Code::BadContent,
        Code::BadNeeds,
        Code::BadGroup,
        Code::BadTargets,
    ];
}
