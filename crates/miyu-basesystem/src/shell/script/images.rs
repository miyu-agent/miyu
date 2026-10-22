//! 脚本交图（施工 X-35，`docs/blueprint/skills.md`「脚本」第 3 条，2026-10-20 项目主人定走一个目录、不改输出格式）：每次调用
//! 一个空目录，路径交给 `MIYU_SCRIPT_IMAGES_DIR`，脚本把图拷进去；跑完照文件名排，认得的交成图，读完整个目录删掉。
//!
//! - 目录建在缓存目录旁边，随机名，Unix 上 `0700`，之前就在的不用；建好取真实路径：macOS 的临时目录在链接下面
//!   （`/var` → `/private/var`），交给沙盒规格的、自己读的都用它。
//! - 只看这一层，至多看 [`MOST_NAMES`] 个名字；照 [`miyu_fs::open_file`] 打开：路上不跟链接、不阻塞、只认普通文件，开了以后
//!   看大小，超过 [`picture::MAX_BYTES`] 的不读进内存。
//! - 照 [`picture`] 认格式、量尺寸、查上限，至多 [`MOST_IMAGES`] 张。别的不交，运行日志记一行 `WARN script image dropped`，
//!   给写脚本的人查；不给她另写一句（`26-提示词.md` J12）。

use std::ffi::OsString;
use std::hash::{BuildHasher, Hasher};
use std::io::Read;
use std::path::{Path, PathBuf};

use miyu_kernel::id::MediaType;
use miyu_tool::{Picture, picture};

/// 运行日志的模块。
const TARGET: &str = "miyu::shell";

/// 一次至多交几张图。
const MOST_IMAGES: usize = 4;

/// 目录里至多看几个名字：脚本往里塞了很多的，后面的当没有，不往内存里一直读。
const MOST_NAMES: usize = 1024;

/// 随机名撞上了已经在的，至多再试几次。
const TRIES: usize = 8;

/// 这一次调用交图的目录；丢掉就删。
pub(super) struct Folder {
    path: PathBuf,
}

impl Folder {
    /// 在 `cache` 旁边建一个空的。建不成、取不到真实路径的是空的：这一次不给目录，脚本照常跑。
    pub(super) fn create(cache: &Path) -> Option<Folder> {
        let parent = cache.parent()?;
        for _ in 0..TRIES {
            let path = parent.join(format!("images-{:016x}", random()));
            // 只给本人（Unix 上 0700）；Windows 照它自己的权限，不设。
            #[cfg(unix)]
            let builder = {
                let mut builder = std::fs::DirBuilder::new();
                std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
                builder
            };
            #[cfg(not(unix))]
            let builder = std::fs::DirBuilder::new();
            match builder.create(&path) {
                Ok(()) => {
                    // 先包起来：取不到真实路径的，丢掉时删掉刚建的。
                    let mut made = Folder { path };
                    made.path = made.path.canonicalize().ok()?;
                    return Some(made);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(_) => return None,
            }
        }
        None
    }

    /// 目录的真实路径。
    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    /// 收图：照文件名排，认得的至多 [`MOST_IMAGES`] 张交回；别的记一行。收完目录删掉（丢掉时）。`tool` 是工具名，记日志用。
    pub(super) fn collect(self, tool: &str) -> Vec<Picture> {
        let Ok(entries) = std::fs::read_dir(&self.path) else {
            return Vec::new();
        };
        let mut names: Vec<OsString> = entries
            .take(MOST_NAMES)
            .filter_map(|entry| entry.ok().map(|entry| entry.file_name()))
            .collect();
        names.sort();
        let mut pictures = Vec::new();
        for name in names {
            if pictures.len() == MOST_IMAGES {
                dropped(tool, "more than 4 images");
                continue;
            }
            match read(&self.path.join(&name)) {
                Ok(picture) => pictures.push(picture),
                Err(why) => dropped(tool, why),
            }
        }
        pictures
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        // 删不掉的留在系统临时目录里，不影响这一次的结果。
        drop(std::fs::remove_dir_all(&self.path));
    }
}

/// 读一张：不是图、超了上限的交回原因（写死的一句，不带文件名）。
fn read(path: &Path) -> Result<Picture, &'static str> {
    const UNREADABLE: &str = "not readable";
    let file = miyu_fs::open_file(path).map_err(|_| UNREADABLE)?;
    let size = file.metadata().map_err(|_| UNREADABLE)?.len();
    if size > picture::MAX_BYTES {
        return Err("larger than 5 MiB");
    }
    let mut bytes = Vec::new();
    // 开了以后还在长的，读到上限多一个字节就停，下面 `fits` 照字节数拦下。
    file.take(picture::MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| UNREADABLE)?;
    let head = &bytes[..bytes.len().min(picture::HEAD)];
    let kind = picture::kind(head).ok_or("not PNG, JPEG, GIF or WebP")?;
    let media_type = MediaType::parse(kind).map_err(|_| "not PNG, JPEG, GIF or WebP")?;
    let (width, height) = picture::measure(&bytes).ok_or("its size can't be read")?;
    if !picture::fits(bytes.len() as u64, width, height) {
        return Err("larger than 5 MiB or 8000 pixels a side");
    }
    Ok(Picture {
        bytes,
        media_type,
        width,
        height,
    })
}

/// 没交的一个记一行：工具名、原因。文件名是脚本起的，不写（施工 LG-1 上）。
fn dropped(tool: &str, why: &str) {
    tracing::warn!(target: TARGET, tool, why, "script image dropped");
}

/// 随机的 64 位：标准库的散列种子取自系统的随机数，每造一个换一个，三个平台一样，不另加依赖。
fn random() -> u64 {
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u32(std::process::id());
    hasher.finish()
}

#[cfg(test)]
mod tests;
