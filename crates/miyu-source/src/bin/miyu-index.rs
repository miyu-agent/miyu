//! `miyu-index`：软件源收录用的小程序（施工 G-2 上，设计 `37-软件源.md` 第三节）。跑在 `miyu-agent/packages` 的 Actions 里、
//! 项目主人的机器上，不随 `miyu` 发。
//!
//! - `miyu-index keygen <私钥文件>`：新建一对钥匙，私钥写进文件（已经有的不覆盖；Unix 上只给自己读写），印公钥编号、公钥。
//! - `miyu-index sign <index.json> [--key <私钥文件>]`：先照客户端那一套读一遍索引，读不成的不签；私钥从环境变量
//!   `MIYU_INDEX_KEY` 读，没有的读 `--key` 那个文件；写 `<index.json>.sig`。
//! - `miyu-index verify <index.json> [--public <base64>]…`：照旁边的签名文件验；不写 `--public` 的照编进来的官方公钥。
//! - `miyu-index build <规则目录> <输出目录>`（施工 G-2 下）：照规则目录里的四份 TOML 收一趟，写 `<输出目录>/index.json`（没签名，
//!   之后跑 `sign`），新占的编号写回 `names.toml`，包自带的图标写进 `<输出目录>/logos/`（施工 F-18 下，随索引一起发），一行一件事印出这一趟收了什么、跳过了什么。GitHub 的令牌照环境变量
//!   `GITHUB_TOKEN`，只发给 API 的 `/repos/` 下面；API 的地址默认 `https://api.github.com`，环境变量 `MIYU_INDEX_API` 能换（测试用）。
//!
//! 成了退出码 0，不成 1（原因印在标准错误上），参数不对 2。

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use std::time::Duration;

use miyu_http::{Get, Got, Proxy};
use miyu_source::collect::{self, Fetch, Rules};
use miyu_source::index::Index;
use miyu_source::sign::{self, PublicKey};

/// 环境变量：私钥（Actions 的密钥）。
const KEY_VAR: &str = "MIYU_INDEX_KEY";

const USAGE: &str = "usage: miyu-index build <rules-dir> <out-dir>\n       miyu-index keygen <secret-file>\n       miyu-index sign <index.json> [--key <secret-file>]\n       miyu-index verify <index.json> [--public <base64>]...";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let done = match args.first().map(String::as_str) {
        Some("build") => match &args[1..] {
            [rules, out] => build(Path::new(rules), Path::new(out)),
            _ => return usage(),
        },
        Some("keygen") => match &args[1..] {
            [file] => keygen(Path::new(file)),
            _ => return usage(),
        },
        Some("sign") => match &args[1..] {
            [index] => sign_file(Path::new(index), std::env::var(KEY_VAR).ok(), None),
            [index, flag, key] if flag == "--key" => sign_file(
                Path::new(index),
                std::env::var(KEY_VAR).ok(),
                Some(Path::new(key)),
            ),
            _ => return usage(),
        },
        Some("verify") => {
            let Some((index, rest)) = args[1..].split_first() else {
                return usage();
            };
            let Some(publics) = publics(rest) else {
                return usage();
            };
            verify_file(Path::new(index), &publics)
        }
        _ => return usage(),
    };
    match done {
        Ok(said) => {
            println!("{said}");
            ExitCode::SUCCESS
        }
        Err(why) => {
            eprintln!("{why}");
            ExitCode::from(1)
        }
    }
}

fn usage() -> ExitCode {
    eprintln!("{USAGE}");
    ExitCode::from(2)
}

/// `--public <base64>` 几对；写错了的没有。
fn publics(rest: &[String]) -> Option<Vec<String>> {
    rest.chunks(2)
        .map(|pair| match pair {
            [flag, key] if flag == "--public" => Some(key.clone()),
            _ => None,
        })
        .collect()
}

/// 新建钥匙，私钥写进 `file`。
fn keygen(file: &Path) -> Result<String, String> {
    let made = sign::generate().map_err(|error| error.to_string())?;
    write_secret(file, &made.secret).map_err(|error| format!("{}: {error}", file.display()))?;
    Ok(format!("key {}\npublic {}", made.id, made.public))
}

/// 私钥写进新文件：已经有的不覆盖，Unix 上只给自己读写。
fn write_secret(file: &Path, secret: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut out = options.open(file)?;
    writeln!(out, "{secret}")?;
    out.sync_all()
}

/// 签 `index`，写旁边的 `.sig`。私钥照 `from_env`、`key_file` 的先后取。
fn sign_file(
    index: &Path,
    from_env: Option<String>,
    key_file: Option<&Path>,
) -> Result<String, String> {
    let secret = match (from_env, key_file) {
        (Some(secret), _) if !secret.trim().is_empty() => secret,
        (_, Some(file)) => {
            std::fs::read_to_string(file).map_err(|error| format!("{}: {error}", file.display()))?
        }
        _ => return Err(format!("no secret key: set {KEY_VAR} or pass --key")),
    };
    let bytes = std::fs::read(index).map_err(|error| format!("{}: {error}", index.display()))?;
    Index::parse(&bytes).map_err(|error| error.to_string())?;
    let signed = sign::sign(&bytes, &secret).map_err(|error| error.to_string())?;
    let out = signature_of(index);
    let text = serde_json::to_string(&signed).map_err(|error| error.to_string())?;
    std::fs::write(&out, format!("{text}\n"))
        .map_err(|error| format!("{}: {error}", out.display()))?;
    Ok(format!(
        "signed with key {} into {}",
        signed.key,
        out.display()
    ))
}

/// 验 `index` 旁边的签名：照 `publics`，没写的照官方的。
fn verify_file(index: &Path, publics: &[String]) -> Result<String, String> {
    let keys: Vec<PublicKey> = if publics.is_empty() {
        sign::official()
    } else {
        publics
            .iter()
            .map(|text| {
                PublicKey::from_base64(text).ok_or_else(|| format!("bad public key {text}"))
            })
            .collect::<Result<_, _>>()?
    };
    let bytes = std::fs::read(index).map_err(|error| format!("{}: {error}", index.display()))?;
    let signature = signature_of(index);
    let signed =
        std::fs::read(&signature).map_err(|error| format!("{}: {error}", signature.display()))?;
    let key = sign::verify(&bytes, &signed, &keys).map_err(|error| error.to_string())?;
    Index::parse(&bytes).map_err(|error| error.to_string())?;
    Ok(format!("good signature from key {key}"))
}

/// 索引旁边的签名文件：`<索引>.sig`。
fn signature_of(index: &Path) -> PathBuf {
    let mut name = index.as_os_str().to_owned();
    name.push(".sig");
    PathBuf::from(name)
}

/// 收一趟：读规则、收、写索引、写回 `names.toml`。
fn build(rules_dir: &Path, out: &Path) -> Result<String, String> {
    let rules = Rules::read(rules_dir)?;
    let api =
        std::env::var("MIYU_INDEX_API").unwrap_or_else(|_| "https://api.github.com".to_string());
    let net = Net::new(&api, std::env::var("GITHUB_TOKEN").ok())?;
    let scratch = out.join(".collect");
    std::fs::create_dir_all(&scratch).map_err(|error| format!("{}: {error}", scratch.display()))?;
    let generated = jiff::Timestamp::now()
        .round(jiff::Unit::Second)
        .map_err(|error| error.to_string())?
        .to_string();
    let collected = collect::collect(&rules, &net, &api, &scratch, &generated);
    if let Err(error) = std::fs::remove_dir_all(&scratch) {
        eprintln!("{}: {error}", scratch.display());
    }
    let text = serde_json::to_string_pretty(&collected.index).map_err(|error| error.to_string())?;
    let index = out.join("index.json");
    std::fs::write(&index, format!("{text}\n"))
        .map_err(|error| format!("{}: {error}", index.display()))?;
    if collected.names != rules.names {
        let names = rules_dir.join("names.toml");
        std::fs::write(&names, collect::rules::names_text(&collected.names))
            .map_err(|error| format!("{}: {error}", names.display()))?;
    }
    for (path, bytes) in &collected.logos {
        let file = out.join(path);
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("{}: {error}", parent.display()))?;
        }
        std::fs::write(&file, bytes).map_err(|error| format!("{}: {error}", file.display()))?;
    }
    let mut said = collected.notes;
    said.push(format!(
        "{} packages into {}",
        collected.index.packages.len(),
        index.display()
    ));
    Ok(said.join("\n"))
}

/// 经 HTTP 拿：令牌只发给 API 的 `/repos/` 下面（读 Release 的说明）；下包文件不带它，跳到别的主机也不会带出去。
struct Net {
    runtime: tokio::runtime::Runtime,
    client: reqwest::Client,
    api: String,
    token: Option<String>,
}

impl Net {
    fn new(api: &str, token: Option<String>) -> Result<Net, String> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| error.to_string())?;
        let client =
            miyu_http::fetcher(Proxy::FromEnvironment).map_err(|error| error.to_string())?;
        Ok(Net {
            runtime,
            client,
            api: api.trim_end_matches('/').to_string(),
            token: token.filter(|token| !token.trim().is_empty()),
        })
    }
}

impl Fetch for Net {
    fn get(&self, url: &str, limit: u64) -> Result<Vec<u8>, String> {
        let mut headers = Vec::new();
        if url.starts_with(&format!("{}/repos/", self.api)) {
            headers.push((
                "accept".to_string(),
                "application/vnd.github+json".to_string(),
            ));
            if let Some(token) = &self.token {
                headers.push(("authorization".to_string(), format!("Bearer {token}")));
            }
        }
        let got = self.runtime.block_on(miyu_http::get(Get {
            client: &self.client,
            url,
            headers: &headers,
            etag: None,
            timeout: Duration::from_secs(600),
            limit: usize::try_from(limit).unwrap_or(usize::MAX),
        }));
        match got {
            Ok(Got::Body { bytes, .. }) => Ok(bytes),
            Ok(Got::NotModified) => Err(format!("{url}: not modified")),
            Err(why) => Err(format!("{url}: {why}")),
        }
    }
}
