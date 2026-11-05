//! 打压缩包：tar.gz、zip，照名字排、不带属主和时刻；算 SHA-256。

use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// 把 `dir` 打成 `archive`：里面一层 `name/`，照名字排，不带属主和时刻；`.zip` 结尾的打 zip，别的打 tar.gz。
pub(super) fn pack(dir: &Path, name: &str, archive: &Path) -> Result<(), String> {
    let mut entries = Vec::new();
    walk(dir, Path::new(name), &mut entries)?;
    let file =
        fs::File::create(archive).map_err(|error| format!("{}：{error}", archive.display()))?;
    let failed = |error: io::Error| format!("{}：{error}", archive.display());
    if archive.extension().is_some_and(|ext| ext == "zip") {
        let mut zip = zip::ZipWriter::new(io::BufWriter::new(file));
        for (inside, from) in &entries {
            let inside = slashed(inside);
            match from {
                None => zip
                    .add_directory(format!("{inside}/"), zip_options(0o755))
                    .map_err(|error| failed(io::Error::other(error)))?,
                Some(from) => {
                    zip.start_file(inside, zip_options(mode(from)?))
                        .map_err(|error| failed(io::Error::other(error)))?;
                    let mut source = fs::File::open(from)
                        .map_err(|error| format!("{}：{error}", from.display()))?;
                    io::copy(&mut source, &mut zip).map_err(failed)?;
                }
            }
        }
        zip.finish()
            .map_err(|error| failed(io::Error::other(error)))?
            .flush()
            .map_err(failed)?;
    } else {
        let encoder =
            flate2::write::GzEncoder::new(io::BufWriter::new(file), flate2::Compression::best());
        let mut tar = tar::Builder::new(encoder);
        tar.mode(tar::HeaderMode::Deterministic);
        for (inside, from) in &entries {
            match from {
                None => {
                    let mut header = tar::Header::new_gnu();
                    header.set_entry_type(tar::EntryType::Directory);
                    header.set_mode(0o755);
                    header.set_size(0);
                    header.set_mtime(0);
                    header.set_uid(0);
                    header.set_gid(0);
                    tar.append_data(&mut header, inside, io::empty())
                        .map_err(failed)?;
                }
                Some(from) => {
                    let mut header = tar::Header::new_gnu();
                    let bytes = fs::metadata(from)
                        .map_err(|error| format!("{}：{error}", from.display()))?
                        .len();
                    header.set_entry_type(tar::EntryType::Regular);
                    header.set_mode(mode(from)?);
                    header.set_size(bytes);
                    header.set_mtime(0);
                    header.set_uid(0);
                    header.set_gid(0);
                    let source = fs::File::open(from)
                        .map_err(|error| format!("{}：{error}", from.display()))?;
                    tar.append_data(&mut header, inside, source)
                        .map_err(failed)?;
                }
            }
        }
        tar.into_inner()
            .and_then(|encoder| encoder.finish())
            .and_then(|mut out| out.flush())
            .map_err(failed)?;
    }
    Ok(())
}

pub(super) fn zip_options(mode: u32) -> zip::write::SimpleFileOptions {
    zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(mode)
        .large_file(true)
}

/// 压缩包里写的权限：能跑的（Unix 上带执行位、Windows 上是 `.exe`）`0755`，别的 `0644`。
pub(super) fn mode(file: &Path) -> Result<u32, String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let bits = fs::metadata(file)
            .map_err(|error| format!("{}：{error}", file.display()))?
            .permissions()
            .mode();
        Ok(if bits & 0o111 != 0 { 0o755 } else { 0o644 })
    }
    #[cfg(not(unix))]
    {
        Ok(if file.extension().is_some_and(|ext| ext == "exe") {
            0o755
        } else {
            0o644
        })
    }
}

/// 照名字排列出 `dir` 下的目录（`None`）和文件，压缩包里的位置前面接 `inside`。链接、别的种类报错：搭出来的目录里不该有。
pub(super) fn walk(
    dir: &Path,
    inside: &Path,
    found: &mut Vec<(PathBuf, Option<PathBuf>)>,
) -> Result<(), String> {
    found.push((inside.to_path_buf(), None));
    let mut children: Vec<fs::DirEntry> = fs::read_dir(dir)
        .map_err(|error| format!("{}：{error}", dir.display()))?
        .collect::<Result<_, _>>()
        .map_err(|error| format!("{}：{error}", dir.display()))?;
    children.sort_by_key(fs::DirEntry::file_name);
    for child in children {
        let path = child.path();
        let kind = child
            .file_type()
            .map_err(|error| format!("{}：{error}", path.display()))?;
        let next = inside.join(child.file_name());
        if kind.is_dir() {
            walk(&path, &next, found)?;
        } else if kind.is_file() {
            found.push((next, Some(path)));
        } else {
            return Err(format!("{} 不是普通文件、目录", path.display()));
        }
    }
    Ok(())
}

/// 压缩包里的路径一律用 `/`。
pub(super) fn slashed(path: &Path) -> String {
    path.components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// 文件的 SHA-256（小写十六进制）和字节数。
pub(super) fn digest(file: &Path) -> Result<(String, u64), String> {
    let mut source =
        fs::File::open(file).map_err(|error| format!("{}：{error}", file.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0; 1 << 16];
    let mut bytes = 0u64;
    loop {
        let read = source
            .read(&mut buffer)
            .map_err(|error| format!("{}：{error}", file.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        bytes += read as u64;
    }
    let hex = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    Ok((hex, bytes))
}
