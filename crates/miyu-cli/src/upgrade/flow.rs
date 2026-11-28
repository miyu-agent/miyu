//! `miyu upgrade` 怎么走（`internal/blueprint/cli/upgrade.md`「怎么走」）：认谁装的，脚本装的才自己升——读发行版订阅看有没有
//! 新版（施工 UP-4：不走 GitHub API），下载校验和清单和签名、照编进来的发版公钥验，下载压缩包、对 SHA-256，解开、查一遍、跑新的
//! `miyu --version`，再和装着的对换。哪一步不对都停下，暂存删掉，装着的不动。数据根一个字节都不碰。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use miyu_source::sign::PublicKey;
use sha2::{Digest, Sha256};

use super::feed;
use super::net::{Net, NetError};
use super::plan::{self, Channel, MARKER, Release, Version};
use super::swap::{self, OLD, STAGE};
use super::{Answer, Said, Step, Upgrade};

/// 发版的仓库（设计 59 第三节第 3 条：发版当天旧仓库转进来，叫这个名字）。
pub(crate) const REPOSITORY: &str = "miyu-agent/miyu";

/// 发行版订阅的地址（施工 UP-4）：普通网页，不算 GitHub API 的匿名额度；草稿不在里面，预发布在（[`plan::candidates`] 挑）。
fn feed_url() -> String {
    format!("https://github.com/{REPOSITORY}/releases.atom")
}

/// 一个标签的一个文件的下载地址：按标签拼。
fn download_url(tag: &str, name: &str) -> String {
    format!("https://github.com/{REPOSITORY}/releases/download/{tag}/{name}")
}

/// 没标记的给人的下载地址。
pub(crate) fn download_page() -> String {
    format!("https://github.com/{REPOSITORY}/releases/latest")
}

/// 订阅、清单、签名、压缩包最多多大。
const LIST_LIMIT: usize = 8 << 20;
const SUMS_LIMIT: usize = 1 << 20;
const ARCHIVE_LIMIT: usize = 1 << 30;

/// 走一趟要的：都从外面给，测试换替身。
pub(crate) struct World<'a> {
    /// 跑着的 `miyu` 的真实位置（解开链接的）。
    pub(crate) exe: PathBuf,
    /// 下载。
    pub(crate) net: &'a dyn Net,
    /// 编进来的发版公钥。
    pub(crate) keys: Vec<PublicKey>,
    /// 这一种系统（`linux-x86_64` 这样），发的四种以外的没有。
    pub(crate) target: Option<&'static str>,
    /// 程序名的后缀：Windows 上 `.exe`。
    pub(crate) exe_suffix: &'static str,
    /// 现在这一版。
    pub(crate) current: Version,
    /// 跑 `<程序> --version`，交回它印的那一行。
    pub(crate) version_of: &'a dyn Fn(&Path) -> Result<String, String>,
    /// 问人要不要升：从哪一版到哪一版。
    pub(crate) ask: &'a mut dyn FnMut(&Version, &Version) -> Answer,
    /// 走到哪一步了，说给人听。
    pub(crate) step: &'a mut dyn FnMut(Step),
    /// 挪一样东西（真跑的是 `fs::rename`）。
    pub(crate) rename: &'a mut dyn FnMut(&Path, &Path) -> io::Result<()>,
}

/// 走一趟，交回要说的那一句。
pub(crate) fn run(args: &Upgrade, world: &mut World<'_>) -> Said {
    let Some(dir) = world.exe.parent().map(Path::to_path_buf) else {
        return Said::Failed("locate".to_string(), world.exe.display().to_string());
    };
    let marker = dir.join(MARKER);
    match fs::read_to_string(&marker) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Said::Unmarked(download_page());
        }
        Err(error) => return Said::BadMarker(marker, error.to_string()),
        Ok(text) => match plan::channel(&text) {
            Err(why) => return Said::BadMarker(marker, why),
            Ok(channel @ (Channel::Aur | Channel::Brew)) => return Said::PackageManager(channel),
            Ok(Channel::Script) => {}
        },
    }
    swap::tidy(&dir);
    if world.keys.is_empty() {
        return Said::NoKeys;
    }
    let Some(target) = world.target else {
        return Said::UnknownSystem(download_page());
    };
    let feed = match world.net.get(&feed_url(), LIST_LIMIT, true) {
        Ok(bytes) => bytes,
        Err(error) => return Said::Net(error),
    };
    let Some(tags) = feed::tags(&feed) else {
        return Said::BadFeed(feed_url());
    };
    let candidates = plan::candidates(&tags, args.prerelease);
    if candidates.is_empty() {
        // 没有正式版、有预发布的说一声加 --prerelease（施工 RL-4补2）。
        return match !args.prerelease && plan::has_pre(&tags) {
            true => Said::OnlyPre,
            false => Said::NoRelease,
        };
    }
    let (release, sums) = match usable(world, &candidates) {
        Ok(Some(found)) => found,
        Ok(None) => return Said::UpToDate(world.current.to_string()),
        Err(said) => return said,
    };
    let name = plan::archive_name(&release.version, target);
    let signature = match fetch(world, &release.tag, "SHA256SUMS.sig", SUMS_LIMIT) {
        Ok(bytes) => bytes,
        Err(said) => return said,
    };
    if let Err(error) = miyu_source::sign::verify(&sums, &signature, &world.keys) {
        return Said::BadSignature(error.to_string());
    }
    let Ok(sums) = String::from_utf8(sums) else {
        return Said::BadSums("not UTF-8".to_string());
    };
    if let Err(why) = miyu_source::sums::check(&sums) {
        return Said::BadSums(why);
    }
    let Some(hash) = plan::hash_of(&sums, &name) else {
        return Said::MissingAsset(format!("SHA256SUMS: {name}"));
    };
    // 清单、签名核对过了才问（施工 UP-4）：问的是一个真能升的版本；压缩包几十 MB，答了升再下。
    if !args.yes {
        match (world.ask)(&world.current, &release.version) {
            Answer::Yes => {}
            Answer::No => return Said::Cancelled,
            Answer::Closed => return Said::Unconfirmed,
        }
    }

    let stage = dir.join(STAGE);
    match fs::create_dir(&stage) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
            return Said::NotWritable(dir);
        }
        Err(error) => return Said::Failed("stage".to_string(), error.to_string()),
    }
    let said = staged(
        world,
        &dir,
        &stage,
        &release.tag,
        &name,
        &hash,
        &release.version,
        target,
    );
    if fs::remove_dir_all(&stage).is_err() {
        // 删不掉的下一次开头再删（`swap::tidy`）。
    }
    said
}

/// 从新到旧找第一个比现在新、带了文件的发布，连同它的校验和清单（施工 UP-4）：`SHA256SUMS` 回 404 的（作废的、没传包的）跳过、
/// 接着往旧的找；碰到不比现在新的就停，交回没有（只升不降）。别的没拿到的停下说。
fn usable(world: &World<'_>, candidates: &[Release]) -> Result<Option<(Release, Vec<u8>)>, Said> {
    for release in candidates {
        if release.version <= world.current {
            return Ok(None);
        }
        let url = download_url(&release.tag, "SHA256SUMS");
        match world.net.get(&url, SUMS_LIMIT, false) {
            Ok(sums) => return Ok(Some((release.clone(), sums))),
            Err(NetError::Status(404)) => {}
            Err(error) => return Err(Said::Net(error)),
        }
    }
    Ok(None)
}

/// 下载这个标签的一个文件：404 的是发布里少了它。
fn fetch(world: &World<'_>, tag: &str, name: &str, limit: usize) -> Result<Vec<u8>, Said> {
    match world.net.get(&download_url(tag, name), limit, false) {
        Ok(bytes) => Ok(bytes),
        Err(NetError::Status(404)) => Err(Said::MissingAsset(name.to_string())),
        Err(error) => Err(Said::Net(error)),
    }
}

/// 有了暂存目录以后的几步：下载、对哈希、解开、查、对换。暂存由调用的一方删。
#[allow(
    clippy::too_many_arguments,
    reason = "一趟里依次用到的几样，拆成结构体反而难读"
)]
fn staged(
    world: &mut World<'_>,
    dir: &Path,
    stage: &Path,
    tag: &str,
    name: &str,
    hash: &str,
    version: &Version,
    target: &str,
) -> Said {
    (world.step)(Step::Downloading(name.to_string()));
    let bytes = match fetch(world, tag, name, ARCHIVE_LIMIT) {
        Ok(bytes) => bytes,
        Err(said) => return said,
    };
    let got: String = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if got != hash {
        return Said::HashMismatch(name.to_string());
    }
    (world.step)(Step::Checking);
    let file = stage.join(name);
    if let Err(error) = fs::write(&file, &bytes) {
        return Said::Failed("write".to_string(), error.to_string());
    }
    let unpacked = stage.join("unpacked");
    if let Err(why) = swap::unpack(&file, &unpacked) {
        return Said::Failed("unpack".to_string(), why);
    }
    let top = unpacked.join(format!("miyu-{version}-{target}"));
    let missing = swap::missing(&top, world.exe_suffix);
    if !missing.is_empty() {
        return Said::BadPackage(missing);
    }
    let expected = format!("miyu {version}");
    match (world.version_of)(&top.join(format!("miyu{}", world.exe_suffix))) {
        Ok(said) if said.trim() == expected => {}
        Ok(said) => {
            return Said::WrongVersion {
                expected,
                said: said.trim().to_string(),
            };
        }
        Err(why) => return Said::Failed("run".to_string(), why),
    }
    (world.step)(Step::Replacing);
    let old = dir.join(format!("{OLD}{}", world.current));
    match swap::swap(dir, &top, &old, world.exe_suffix, world.rename) {
        Ok(swapped) => Said::Upgraded {
            from: world.current.to_string(),
            to: version.to_string(),
            left: swapped.left,
            kept: swapped.kept,
        },
        Err((step, why)) => Said::Failed(step, why),
    }
}
