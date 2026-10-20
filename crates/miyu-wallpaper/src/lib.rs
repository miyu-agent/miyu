//! 读这台电脑现在的桌面壁纸（蓝图 `web.md`「主题」的「跟着人格的外观」第 5 条）：主题「跟随壁纸」从它取色。浏览器读不到桌面，由和桌面在
//! 同一台机器上的网页服务（现在是演示的桥 `web-demo/bridge`）问这里，再交给页面。两个来处，网页服务先问 [`matugen`]：
//! - [`matugen`]：用 matugen 换壁纸的，照仓库给的模板生成到固定地方的那一份颜色；
//! - [`find`]：自己照各家桌面找壁纸（先后见 `probe.rs`、`unix.rs`）：
//!   - Linux：KDE、GNOME、XFCE、Cinnamon、MATE 的设置，awww、swww、hyprpaper、swaybg、wpaperd、waypaper，X11 上的 feh、nitrogen；
//!   - macOS：系统存壁纸设置的 `Index.plist`，读不到的问 System Events（第一次会问要不要允许）；
//!   - Windows：注册表的 `WallPaper`，读不到的用 Windows 转好的 `TranscodedWallpaper`。
//!
//! 找到的还要是浏览器认得的图（照文件开头几个字节认：PNG、JPEG、WebP、GIF、BMP、AVIF）；macOS 上的 HEIC 用系统自带的 `sips` 转成 JPEG，
//! 别的（视频壁纸）当读不到。只用标准库；跑的命令都有时限（[`TIMEOUT`]），没装、没在跑、出错退出的当这一家没有，接着问下一家。读各家
//! 回答的纯函数在 [`parse`]（每个平台上都编、都测）。
//!
//! 换了壁纸要跟着变的，照 [`watch`] 盯文件（[`Wallpaper::watch`]、没找到时的 [`watch::watch_list`]），不定时去问。

pub mod matugen;
pub mod parse;
mod probe;
#[cfg(test)]
mod tests;
#[cfg(all(unix, not(target_os = "macos")))]
mod unix;
pub mod watch;

use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

/// 跑一条命令最多等多久：壁纸程序卡住了也不能把页面的请求挂着。
pub const TIMEOUT: Duration = Duration::from_secs(2);

/// 壁纸最大多大：再大的不读（8K 的 PNG 也到不了）。
pub const MAX_BYTES: u64 = 64 * 1024 * 1024;

/// 找到的壁纸。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wallpaper {
    /// 图的绝对路径。
    pub path: PathBuf,
    /// 照哪一家找到的（`kde`、`gnome`、`awww`、`swaybg`、`macos`、`windows`……），排查用。
    pub from: &'static str,
    /// 交给浏览器的媒体类型（照文件开头认的；要转的是转出来的 JPEG）。
    pub mime: &'static str,
    /// 是 HEIC，读的时候先转成 JPEG（只在 macOS 上有）。
    pub heic: bool,
    /// 要盯哪些文件，变了就重找（[`watch::Watch`]）：图本身、找到它的那一家的文件（[`watch::files_of`]），加上 [`watch::watch_list`]。
    pub watch: Vec<PathBuf>,
}

impl Wallpaper {
    /// 换没换的记号：路径、大小、改动时间合成一串；同一张图没动过的一样（只在这一个进程里比）。
    ///
    /// # Errors
    ///
    /// 读不了文件的信息（被删了、没权限）。
    pub fn signature(&self) -> io::Result<String> {
        signature_of(&self.path)
    }

    /// 读整张图。
    ///
    /// # Errors
    ///
    /// 读不了、超过 [`MAX_BYTES`]。
    pub fn read(&self) -> io::Result<Vec<u8>> {
        if self.heic {
            heic_to_jpeg(&self.path)
        } else {
            read_capped(&self.path, MAX_BYTES)
        }
    }
}

/// 换没换的记号：路径、大小、改动时间合成一串（只在这一个进程里比）。
pub(crate) fn signature_of(path: &Path) -> io::Result<String> {
    let meta = std::fs::metadata(path)?;
    let mut h = DefaultHasher::new();
    path.hash(&mut h);
    meta.len().hash(&mut h);
    meta.modified()?.hash(&mut h);
    Ok(format!("{:016x}", h.finish()))
}

/// 读整个文件，超过 `max` 字节的不读。
pub(crate) fn read_capped(path: &Path, max: u64) -> io::Result<Vec<u8>> {
    let len = std::fs::metadata(path)?.len();
    if len > max {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{} 太大：{len} 字节", path.display()),
        ));
    }
    std::fs::read(path)
}

/// HEIC 转 JPEG：macOS 自带的 `sips` 转到临时文件，读回来、删掉。
#[cfg(target_os = "macos")]
fn heic_to_jpeg(path: &Path) -> io::Result<Vec<u8>> {
    let out = std::env::temp_dir().join(format!("miyu-wallpaper-{}.jpg", std::process::id()));
    let target = out.to_string_lossy().into_owned();
    let source = path.to_string_lossy().into_owned();
    let done = run("sips", &["-s", "format", "jpeg", &source, "--out", &target])?;
    let bytes = match done {
        Some(_) => std::fs::read(&out),
        None => Err(io::Error::other("sips 转不了这张 HEIC")),
    };
    if out.exists() {
        std::fs::remove_file(&out)?;
    }
    bytes
}

/// 别的平台上不会有要转的（`find` 只在 macOS 上认 HEIC）。
#[cfg(not(target_os = "macos"))]
fn heic_to_jpeg(_path: &Path) -> io::Result<Vec<u8>> {
    Err(io::Error::other("只在 macOS 上转 HEIC"))
}

/// 现在的桌面壁纸：照这台电脑的桌面一家家问，第一张浏览器认得的图。
///
/// `dark` 是页面这时是不是深色（GNOME、KDE 深浅可以各设一张）。
///
/// # Errors
///
/// 都没找到、中间有一家出了错（跑不了命令、超时、读不了配置文件）的交那个错，排查用；一家家没装、没设的不算错，交 `Ok(None)`。
pub fn find(dark: bool) -> io::Result<Option<Wallpaper>> {
    let mut failed = None;
    for (from, ask) in probe::sources() {
        match ask(dark) {
            Ok(Some(path)) => match image_kind(&path) {
                Ok(Some((mime, heic))) => {
                    let mut watch = vec![path.clone()];
                    watch.extend(watch::files_of(from));
                    watch.extend(watch::watch_list());
                    return Ok(Some(Wallpaper {
                        path,
                        from,
                        mime,
                        heic,
                        watch,
                    }));
                }
                Ok(None) => {}
                Err(e) => failed = failed.or(Some(e)),
            },
            Ok(None) => {}
            Err(e) => failed = failed.or(Some(io::Error::new(e.kind(), format!("{from}：{e}")))),
        }
    }
    failed.map_or(Ok(None), Err)
}

/// 一个文件是不是浏览器认得的图：读开头几个字节认，交（给浏览器的媒体类型，要不要先从 HEIC 转）；不是普通文件、认不出的是 `None`。
/// HEIC 只在 macOS 上认（有 `sips` 转）。
fn image_kind(path: &Path) -> io::Result<Option<(&'static str, bool)>> {
    if !path.is_file() {
        return Ok(None);
    }
    let mut head = [0u8; 16];
    let mut file = std::fs::File::open(path)?;
    let n = file.read(&mut head)?;
    let head = &head[..n];
    if let Some(mime) = parse::sniff(head) {
        return Ok(Some((mime, false)));
    }
    Ok((cfg!(target_os = "macos") && parse::is_heic(head)).then_some(("image/jpeg", true)))
}

/// 跑一条命令、读它的标准输出。没装的（找不到程序）、出错退出的是 `None`；超时的杀掉，算错。不定时去看它退没退（2026-10-15 设计 00
/// 「避免轮询」）：另一个线程把输出读到头（它退了、管子就关了），这边照时限等那一个线程交回。
pub(crate) fn run(program: &str, args: &[&str]) -> io::Result<Option<String>> {
    let mut child = match Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let (read, got) = mpsc::channel();
    if let Some(mut pipe) = child.stdout.take() {
        std::thread::spawn(move || {
            let mut out = String::new();
            let _gone = read.send(pipe.read_to_string(&mut out).map(|_| out));
        });
    }
    let out = match got.recv_timeout(TIMEOUT) {
        Ok(out) => out,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            child.kill()?;
            child.wait()?;
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("{program} 过了 {} 秒没回", TIMEOUT.as_secs()),
            ));
        }
        // 没有管子（不会发生）：照没有输出
        Err(mpsc::RecvTimeoutError::Disconnected) => Ok(String::new()),
    };
    if !child.wait()?.success() {
        return Ok(None);
    }
    out.map(Some)
}
