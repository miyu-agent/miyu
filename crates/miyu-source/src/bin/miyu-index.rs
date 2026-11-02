//! `miyu-index`：软件源收录用的小程序（施工 G-2 上，设计 `37-软件源.md` 第三节）。跑在 `miyu-agent/packages` 的 Actions 里、
//! 项目主人的机器上，不随 `miyu` 发。
//!
//! - `miyu-index keygen <私钥文件>`：新建一对钥匙，私钥写进文件（已经有的不覆盖；Unix 上只给自己读写），印公钥编号、公钥。
//! - `miyu-index sign <index.json> [--key <私钥文件>]`：先照客户端那一套读一遍索引，读不成的不签；私钥从环境变量
//!   `MIYU_INDEX_KEY` 读，没有的读 `--key` 那个文件；写 `<index.json>.sig`。
//! - `miyu-index verify <index.json> [--public <base64>]…`：照旁边的签名文件验；不写 `--public` 的照编进来的官方公钥。
//! - `miyu-index build <规则目录> <输出目录> [--github]`（施工 G-2 下）：照规则目录里的四份 TOML 收一趟，写 `<输出目录>/index.json`（没签名，
//!   之后跑 `sign`），新占的编号写回 `names.toml`，包自带的图标写进 `<输出目录>/logos/`（施工 F-18 下，随索引一起发），一行一件事印出这一趟收了什么、跳过了什么。GitHub 的令牌照环境变量
//!   `GITHUB_TOKEN`，只发给 API 的 `/repos/`、`/search/` 下面（后一个是社区来源搜话题，施工 CC-1）；API 的地址默认 `https://api.github.com`，环境变量 `MIYU_INDEX_API` 能换（测试用）。
//!   带 `--github` 的（施工 DL-2，设计 54）再问每个包的来源仓库全部发布的下载次数，记进 `<输出目录>/stats.json`（最近 8 天，
//!   不签名），写进索引的 `downloads`、`downloads_week`；问不成的照上一份索引沿用，不挡生成。没有 `GITHUB_TOKEN` 的照匿名问，
//!   一小时只有 60 次。
//! - `miyu-index folder <包文件夹> <输出目录> --url <源的地址>`（施工 G-13，自建的软件源）：把一个文件夹里的 `.miyu` 收成索引，包文件拷到
//!   `<输出目录>/files/`，写 `<输出目录>/index.json`（没签名，之后跑 `sign`）和图标。整个输出目录照原样放到 `<源的地址>` 下面
//!   （任何静态的网页服务都行），用的人在配置里加 `packages.sources.<名字>`，写这个地址和签名的公钥。
//!
//! - `build`、`folder` 写 `index.json` 之前先读输出目录里的上一份，沿用每个包的 `published`（[`stats::carry_published`]）。
//!
//! 成了退出码 0，不成 1（原因印在标准错误上），参数不对 2。

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use std::time::Duration;

use miyu_http::{Get, Got, Proxy};
use miyu_source::collect::{self, Fetch, Rules};
use miyu_source::index::Index;
use miyu_source::sign::{self, PublicKey};
use miyu_source::stats::{self, Stats};

/// 环境变量：私钥（Actions 的密钥）。
const KEY_VAR: &str = "MIYU_INDEX_KEY";

const USAGE: &str = "usage: miyu-index build <rules-dir> <out-dir> [--github]\n       miyu-index folder <package-dir> <out-dir> --url <source-url>\n       miyu-index keygen <secret-file>\n       miyu-index sign <index.json> [--key <secret-file>]\n       miyu-index verify <index.json> [--public <base64>]...";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let done = match args.first().map(String::as_str) {
        Some("build") => match &args[1..] {
            [rules, out] => build(Path::new(rules), Path::new(out), false),
            [rules, out, flag] if flag == "--github" => {
                build(Path::new(rules), Path::new(out), true)
            }
            _ => return usage(),
        },
        Some("folder") => match &args[1..] {
            [folder, out, flag, url] if flag == "--url" => {
                build_folder(Path::new(folder), Path::new(out), url)
            }
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

/// 收一趟：读规则、收、写索引、写回 `names.toml`；`github` 的再数下载次数、写 `stats.json`。
fn build(rules_dir: &Path, out: &Path, github: bool) -> Result<String, String> {
    let rules = Rules::read(rules_dir)?;
    let api =
        std::env::var("MIYU_INDEX_API").unwrap_or_else(|_| "https://api.github.com".to_string());
    let net = Net::new(&api, std::env::var("GITHUB_TOKEN").ok())?;
    let scratch = out.join(".collect");
    std::fs::create_dir_all(&scratch).map_err(|error| format!("{}: {error}", scratch.display()))?;
    let generated = now()?;
    let mut collected = collect::collect(&rules, &net, &api, &scratch, &generated);
    if let Err(error) = std::fs::remove_dir_all(&scratch) {
        eprintln!("{}: {error}", scratch.display());
    }
    if collected.names != rules.names {
        let names = rules_dir.join("names.toml");
        std::fs::write(&names, collect::rules::names_text(&collected.names))
            .map_err(|error| format!("{}: {error}", names.display()))?;
    }
    let previous = previous(out, &mut collected.notes);
    let counted = if github {
        let date = generated
            .parse::<jiff::Timestamp>()
            .map_err(|error| error.to_string())?
            .to_zoned(jiff::tz::TimeZone::UTC)
            .date();
        let file = out.join("stats.json");
        let mut stats = match std::fs::read(&file) {
            Ok(bytes) => Stats::parse(&bytes).unwrap_or_else(|| {
                collected
                    .notes
                    .push(format!("{}: unreadable, starting over", file.display()));
                Stats::default()
            }),
            Err(_) => Stats::default(),
        };
        let notes = stats::count(
            &mut collected.index,
            previous.as_ref(),
            &mut stats,
            date,
            (&net, &api),
        );
        collected.notes.extend(notes);
        Some((file, stats))
    } else {
        None
    };
    let said = write_out(out, collected, previous.as_ref())?;
    if let Some((file, stats)) = counted {
        let text = serde_json::to_string_pretty(&stats).map_err(|error| error.to_string())?;
        std::fs::write(&file, format!("{text}\n"))
            .map_err(|error| format!("{}: {error}", file.display()))?;
    }
    Ok(said)
}

/// 输出目录里上一份 `index.json`：没有的没有；读不成的也没有，记一笔（上线时刻从这一次算起）。
fn previous(out: &Path, notes: &mut Vec<String>) -> Option<Index> {
    let file = out.join("index.json");
    let bytes = std::fs::read(&file).ok()?;
    Index::parse(&bytes)
        .map_err(|error| {
            notes.push(format!(
                "{}: last index unreadable, {error}; published starts over",
                file.display()
            ));
        })
        .ok()
}

/// 从一个文件夹收（施工 G-13）：收成索引，收了的包文件拷进 `<out>/files/`。
fn build_folder(folder: &Path, out: &Path, url: &str) -> Result<String, String> {
    let scratch = out.join(".collect");
    std::fs::create_dir_all(&scratch).map_err(|error| format!("{}: {error}", scratch.display()))?;
    let mut collected = collect::from_folder(folder, url, &scratch, &now()?)?;
    if let Err(error) = std::fs::remove_dir_all(&scratch) {
        eprintln!("{}: {error}", scratch.display());
    }
    let files = out.join("files");
    std::fs::create_dir_all(&files).map_err(|error| format!("{}: {error}", files.display()))?;
    for entry in &collected.index.packages {
        for file in &entry.files {
            let name = file.url.rsplit('/').next().unwrap_or_default();
            std::fs::copy(folder.join(name), files.join(name))
                .map_err(|error| format!("{name}: {error}"))?;
        }
    }
    let previous = previous(out, &mut collected.notes);
    write_out(out, collected, previous.as_ref())
}

/// 这一刻，照秒取整：写进索引的 `generated`。
fn now() -> Result<String, String> {
    Ok(jiff::Timestamp::now()
        .round(jiff::Unit::Second)
        .map_err(|error| error.to_string())?
        .to_string())
}

/// 照上一份 `previous` 沿用 `published`，写 `<out>/index.json` 和图标，交回这一趟收了什么。
fn write_out(
    out: &Path,
    mut collected: collect::Collected,
    previous: Option<&Index>,
) -> Result<String, String> {
    stats::carry_published(&mut collected.index, previous);
    let text = serde_json::to_string_pretty(&collected.index).map_err(|error| error.to_string())?;
    let index = out.join("index.json");
    std::fs::write(&index, format!("{text}\n"))
        .map_err(|error| format!("{}: {error}", index.display()))?;
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

/// 经 HTTP 拿：令牌只发给 API 的 `/repos/`（读 Release 的说明）、`/search/`（搜话题，匿名一分钟只能搜 10 次）下面；下包文件
/// 不带它，跳到别的主机也不会带出去。
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
        if ["/repos/", "/search/"]
            .iter()
            .any(|path| url.starts_with(&format!("{}{path}", self.api)))
        {
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
