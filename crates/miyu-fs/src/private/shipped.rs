//! 出厂的私密目录（施工 5-13 起，5-17 上照各家对照补齐，2026-10-21 项目主人定）：钥匙、云和集群的凭据、明文令牌、密码库和
//! 钥匙环、浏览器、别的 agent 的登录、旧版 Miyu 的数据。写成 `~/…`；有的是文件（`~/.netrc` 这种）。不管在不在。
//!
//! 头五个三个平台都一样、先后不变：`~/.ssh`、`~/.gnupg`、`~/.claude`、`~/.miyu`、`~/.config/gh`。

/// Linux：照对照报告的推荐（`私密目录-各家对照.md`「推荐的 Miyu 出厂清单」）。
#[cfg(target_os = "linux")]
pub const SHIPPED_PRIVATE: &[&str] = &[
    "~/.ssh",
    "~/.gnupg",
    "~/.claude",
    "~/.miyu",
    "~/.config/gh",
    // 别的 agent 的登录。
    "~/.codex",
    "~/.gemini",
    // 云和集群的凭据。
    "~/.aws",
    "~/.azure",
    "~/.config/gcloud",
    "~/.oci",
    "~/.kube",
    "~/.docker",
    // 明文令牌。
    "~/.git-credentials",
    "~/.config/git/credentials",
    "~/.netrc",
    "~/.pypirc",
    "~/.cargo/credentials.toml",
    "~/.gem/credentials",
    "~/.vault-token",
    "~/.terraform.d",
    "~/.pgpass",
    // 密码库和桌面钥匙环。
    "~/.password-store",
    "~/.local/share/keyrings",
    "~/.local/share/kwalletd",
    // 浏览器、邮件：各渠道版，Flatpak、Snap 装的。
    "~/.mozilla",
    "~/.thunderbird",
    "~/.config/google-chrome",
    "~/.config/google-chrome-beta",
    "~/.config/google-chrome-unstable",
    "~/.config/chromium",
    "~/.config/BraveSoftware",
    "~/.config/microsoft-edge",
    "~/.config/vivaldi",
    "~/.config/opera",
    "~/.var/app/org.mozilla.firefox",
    "~/.var/app/org.mozilla.Thunderbird",
    "~/.var/app/com.google.Chrome",
    "~/.var/app/org.chromium.Chromium",
    "~/.var/app/com.brave.Browser",
    "~/.var/app/com.microsoft.Edge",
    "~/.var/app/com.vivaldi.Vivaldi",
    "~/.var/app/com.opera.Opera",
    "~/snap/firefox",
    "~/snap/chromium",
];

/// macOS：同 Linux 的凭据，加钥匙串、浏览器、密码管理器。
#[cfg(target_os = "macos")]
pub const SHIPPED_PRIVATE: &[&str] = &[
    "~/.ssh",
    "~/.gnupg",
    "~/.claude",
    "~/.miyu",
    "~/.config/gh",
    "~/.codex",
    "~/.gemini",
    "~/.aws",
    "~/.azure",
    "~/.config/gcloud",
    "~/.kube",
    "~/.docker",
    "~/.git-credentials",
    "~/.netrc",
    "~/.pypirc",
    "~/.cargo/credentials.toml",
    "~/.vault-token",
    "~/Library/Keychains",
    "~/Library/Cookies",
    "~/Library/Safari",
    "~/Library/Application Support/Google/Chrome",
    "~/Library/Application Support/Chromium",
    "~/Library/Application Support/BraveSoftware",
    "~/Library/Application Support/Microsoft Edge",
    "~/Library/Application Support/Vivaldi",
    "~/Library/Application Support/Firefox",
    "~/Library/Application Support/1Password",
    "~/Library/Application Support/Bitwarden",
];

/// Windows：基本照 sandbox-runtime 的清单，两种放凭据的习惯（家目录下、`AppData` 下）都列。
#[cfg(windows)]
pub const SHIPPED_PRIVATE: &[&str] = &[
    "~/.ssh",
    "~/.gnupg",
    "~/.claude",
    "~/.miyu",
    "~/.config/gh",
    "~/.codex",
    "~/.gemini",
    "~/.aws",
    "~/.azure",
    "~/.config/gcloud",
    "~/.kube",
    "~/.docker",
    "~/.terraform.d",
    "~/.git-credentials",
    "~/_netrc",
    "~/.netrc",
    "~/.pypirc",
    "~/.cargo/credentials.toml",
    "~/.vault-token",
    "~/AppData/Roaming/gnupg",
    "~/AppData/Roaming/GitHub CLI",
    "~/AppData/Roaming/gcloud",
    // 浏览器。
    "~/AppData/Local/Google/Chrome/User Data",
    "~/AppData/Local/Microsoft/Edge/User Data",
    "~/AppData/Local/BraveSoftware/Brave-Browser/User Data",
    "~/AppData/Local/Chromium/User Data",
    "~/AppData/Local/Vivaldi/User Data",
    "~/AppData/Roaming/Mozilla/Firefox",
    "~/AppData/Roaming/Opera Software",
    // 密码管理器。
    "~/AppData/Roaming/1Password",
    "~/AppData/Roaming/Bitwarden",
    "~/AppData/Roaming/KeePass",
    "~/AppData/Roaming/KeePassXC",
    // 系统的凭据库、账号令牌缓存。
    "~/AppData/Roaming/Microsoft/Credentials",
    "~/AppData/Roaming/Microsoft/Protect",
    "~/AppData/Roaming/Microsoft/Vault",
    "~/AppData/Roaming/Microsoft/Crypto",
    "~/AppData/Roaming/Microsoft/SystemCertificates",
    "~/AppData/Local/Microsoft/Credentials",
    "~/AppData/Local/Microsoft/Protect",
    "~/AppData/Local/Microsoft/Vault",
    "~/AppData/Local/Microsoft/Crypto",
    "~/AppData/Local/Microsoft/SystemCertificates",
    "~/AppData/Local/Microsoft/IdentityCache",
    "~/AppData/Local/Microsoft/TokenBroker",
    "~/AppData/Local/Microsoft/OneAuth",
    // 注册表。商店应用的私有数据 `~/AppData/Local/Packages` 不列（施工 SW-2，2026-10-30 项目主人定）：MXC 的容器自己的目录
    // （临时目录在内）就在它下面，名字每次不一样，禁了整个它，PowerShell 写不进临时目录就退到受限的语言模式，命令一条条失败。
    "~/NTUSER.DAT",
    "~/AppData/Local/Microsoft/Windows/UsrClass.dat",
];

/// 别的 Unix：三个平台都有的那几个，加凭据。
#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
pub const SHIPPED_PRIVATE: &[&str] = &[
    "~/.ssh",
    "~/.gnupg",
    "~/.claude",
    "~/.miyu",
    "~/.config/gh",
    "~/.codex",
    "~/.gemini",
    "~/.aws",
    "~/.azure",
    "~/.config/gcloud",
    "~/.kube",
    "~/.docker",
    "~/.git-credentials",
    "~/.netrc",
    "~/.pypirc",
    "~/.cargo/credentials.toml",
    "~/.vault-token",
];
