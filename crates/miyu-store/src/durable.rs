//! 落盘的几件小事（`docs/designs/07-存储.md` 第四节「各平台的坑」）：同步目录；建目录时，新建的
//! 每一层都同步它的上一层。新建文件、新建目录、改名以后，上一层不同步，断电以后这一项可能没了。
//! 先写临时文件再改名的，临时文件怎么建、叫什么、用不上了怎么删（blob、生成的文件、配置文件共用，施工 8-1 从 `blob.rs`
//! 挪来，名字施工 8-3 从 `generated.rs` 挪来）。
//!
//! 数据根里的东西只在数据根还在的时候建（施工 TMP-1，2026-10-28 主会话定）：要建的目录往上找不到数据根的标记
//! （[`crate::root::MARKER`]）的不建、报错（[`create_dir`]）。数据根被删了（测试的夹具删掉以后后台还在写、人删了还开着的核心的
//! 数据根），不再被后台写的那一下连上级目录一起建回来。建数据根本身、写人指定位置的配置文件用不认标记的
//! [`create_dir_anywhere`]。

use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// 临时文件的名字最多换几次：撞上的都是崩溃留下的，换几次总能换开。
const TEMP_TRIES: u32 = 64;

/// 在数据根里建目录，连同缺的上层（同 [`create_dir_anywhere`]）：已经有的不动；要建的，往上（连它自己）找得到数据根的
/// 标记才建，只建标记那一层下面的，数据根本身不建。
///
/// 找到标记和建之间数据根被删掉了的（测试的夹具删了数据根，后台开库的这一下正好夹在中间），不连数据根一起建回来，报
/// `NotFound`（施工 TMP-1 再补：门禁「临时目录」偶尔剩一个只有 `index/recall/turns-_none.db` 的数据根）。
///
/// # Errors
///
/// 往上找不到数据根的标记、数据根已经不在了（`NotFound`，说的是哪个目录）；别的同 [`create_dir_anywhere`]。
pub(crate) fn create_dir(dir: &Path) -> io::Result<()> {
    if dir.is_dir() {
        return Ok(());
    }
    let Some(root) = data_root_of(dir) else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "{} is not inside a Miyu data root (no {} above it), not creating it",
                dir.display(),
                crate::root::MARKER
            ),
        ));
    };
    create_below(root, dir)
}

/// `dir` 往上（连它自己）头一个有数据根标记的那一层。
fn data_root_of(dir: &Path) -> Option<&Path> {
    dir.ancestors()
        .find(|above| fs::symlink_metadata(above.join(crate::root::MARKER)).is_ok())
}

/// 建数据根 `root` 下面缺的几层，一直到 `dir`；`root` 本身不建，不在了的报 `NotFound`。
fn create_below(root: &Path, dir: &Path) -> io::Result<()> {
    if dir == root {
        if root.is_dir() {
            return Ok(());
        }
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "the Miyu data root {} is gone, not creating it again",
                root.display()
            ),
        ));
    }
    if dir.is_dir() {
        return Ok(());
    }
    let parent = dir.parent().unwrap_or(root);
    create_below(root, parent)?;
    make_one(dir, Some(parent))
}

/// 建目录，连同缺的上层，不管在不在数据根里（建数据根本身、写人指定位置的配置文件）。新建的每一层都同步它的上一层，建
/// 目录这件事本身才算落盘；已经有的不动。
///
/// Unix 上新建的权限 0700，只有本人能进；已经有的不改：数据根可能是人自己建、自己设的，权限
/// 不对由 `miyu doctor` 报告（`22-命令行.md` 第五节）。Windows 上靠用户目录本身的访问控制。
///
/// # Errors
///
/// 建不了；该是目录的地方是个文件；同步不了。
pub(crate) fn create_dir_anywhere(dir: &Path) -> io::Result<()> {
    if dir.is_dir() {
        return Ok(());
    }
    let parent = dir.parent().filter(|parent| !parent.as_os_str().is_empty());
    if let Some(parent) = parent {
        create_dir_anywhere(parent)?;
    }
    make_one(dir, parent)
}

/// 上一层 `parent` 已经有了，建 `dir` 这一层（Unix 上 0700），同步上一层；同一时刻别处建好的也算。
fn make_one(dir: &Path, parent: Option<&Path>) -> io::Result<()> {
    #[cfg(unix)]
    let created = {
        use std::os::unix::fs::DirBuilderExt;
        fs::DirBuilder::new().mode(0o700).create(dir)
    };
    #[cfg(not(unix))]
    let created = fs::create_dir(dir);
    match created {
        Ok(()) => {}
        // 同一时刻别处建好的，也算。
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists && dir.is_dir() => {
            return Ok(());
        }
        Err(error) => return Err(error),
    }
    match parent {
        Some(parent) => sync_dir(parent),
        None => Ok(()),
    }
}

/// 同步目录：新建文件、新建目录、改名以后做，这一项本身才算落盘。
///
/// # Errors
///
/// 打不开这个目录，或者同步不了。
#[cfg(unix)]
pub(crate) fn sync_dir(dir: &Path) -> io::Result<()> {
    fs::File::open(dir)?.sync_all()
}

/// Windows 上不用同步目录，也打不开目录来同步。
#[cfg(not(unix))]
pub(crate) fn sync_dir(_dir: &Path) -> io::Result<()> {
    Ok(())
}

/// 在 `dir` 里新建一个临时文件，名字照 `name` 起，只许新建。撞上崩溃留下的同名文件，换下一个名字。
///
/// # Errors
///
/// 建不了；一连 64 个名字都被占了。
pub(crate) fn create_temp(dir: &Path, name: impl FnMut() -> String) -> io::Result<(PathBuf, File)> {
    create_temp_with(dir, name, None)
}

/// 同 [`create_temp`]；给了 `mode` 的 Unix 上建的时候就是这几个权限位（再照 umask 收）：密钥文件的是 0600，只有本人能读写
/// （施工 8-5：不能有一瞬间是别人读得到的）；替换一份配置文件的照原来的（施工 SEC-3，外部审计 L-2：写完以前不比原来的宽）。
/// Windows 上靠目录继承的访问控制，照常建。
///
/// # Errors
///
/// 建不了；一连 64 个名字都被占了。
pub(crate) fn create_temp_with(
    dir: &Path,
    mut name: impl FnMut() -> String,
    mode: Option<u32>,
) -> io::Result<(PathBuf, File)> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    if let Some(mode) = mode {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(mode);
    }
    #[cfg(not(unix))]
    let _ = mode;
    for _ in 0..TEMP_TRIES {
        let path = dir.join(name());
        match options.open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!(
            "{TEMP_TRIES} temporary file names in a row are taken in {}",
            dir.display()
        ),
    ))
}

/// 替换一份文件时的临时文件的名字：点开头（大多数系统上不显示），带上原来的名字 `name`、进程号和一个计数
/// （`.<文件名>.<进程号>-<计数>.tmp`，生成的文件、配置文件共用，施工 8-3 从 `generated.rs` 挪来）。
pub(crate) fn temp_name(name: &str) -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    format!(
        ".{name}.{}-{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

/// 删掉用不上的临时文件。
#[expect(
    clippy::let_underscore_must_use,
    reason = "删不掉就留着，不耽误这一次：blob 的由回收清，生成的文件的留在旁边"
)]
pub(crate) fn discard(temp: &Path) {
    let _ = fs::remove_file(temp);
}

#[cfg(test)]
mod tests;
