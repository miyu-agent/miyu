//! 用 bubblewrap 的挂载收紧（施工 5-16，`docs/blueprint/sandbox/linux.md`「有 bubblewrap 就用它」，2026-10-20 项目主人定：有就用、
//! 不自带）：能写的里面藏着的（放开了的家目录里的数据根、私密目录）用挂载盖住，连名字都看不到，也不冻住路上那几层。挂好以后
//! 在里面再跑一次助手，照内层的规格用 Landlock 收紧：拦系统服务的套接字、能写的以外写不了，同只有 Landlock 时。
//!
//! 只认系统目录里属于 root、别人写不了的 `bwrap`，不照 `PATH` 找：沙盒里的命令写得了 `~/.local/bin`，放一个假的进去，以后
//! 每条命令都等于没关。能不能用照正式的挂载真起一次、验内容（[`usable`]）：起得来不等于挂上了。

use std::ffi::{OsStr, OsString};
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use miyu_sandbox::Spec;

/// 只在这几处找。
const PLACES: [&str; 3] = ["/usr/bin/bwrap", "/bin/bwrap", "/usr/local/bin/bwrap"];

/// 系统目录里属于 root、组和别人都写不了的 `bwrap`。
fn found() -> Option<PathBuf> {
    PLACES.iter().map(PathBuf::from).find(|path| trusted(path))
}

/// `path` 是属于 root、组和别人都写不了的普通文件（跟链接：发行版常把它链到别处，链到的那一个照样查）。
fn trusted(path: &Path) -> bool {
    fs::metadata(path)
        .is_ok_and(|meta| meta.is_file() && meta.uid() == 0 && meta.mode() & 0o022 == 0)
}

/// 照规格 `spec` 拼 bwrap 的挂载（不含程序）。先整盘只读、设备照原样；能写的（不在藏起来的里面的）可写挂回；藏起来的盖一层
/// 空的 tmpfs（是文件的拿 `/dev/null` 盖）；藏起来的里面能写的、放开只读的再挂回；最后藏起来的那几层设成只读，写进去报只读，
/// 不会写进一层用完就丢的内存里。不在的跳过。藏起来的另挂在别处的（挂载别名，施工 SEC-3），别名那一处一样盖住。
fn mounts(spec: &Spec) -> Vec<OsString> {
    mounts_with(spec, &super::aliases::with_aliases(&spec.hidden))
}

/// 同 [`mounts`]，藏起来的照 `hidden`（规格里的连同别名）。
fn mounts_with(spec: &Spec, hidden: &[PathBuf]) -> Vec<OsString> {
    let mut args: Vec<OsString> = [
        "--ro-bind",
        "/",
        "/",
        "--dev-bind",
        "/dev",
        "/dev",
        "--die-with-parent",
    ]
    .into_iter()
    .map(OsString::from)
    .collect();
    let hidden: Vec<&PathBuf> = hidden.iter().filter(|path| path.exists()).collect();
    let inside_hidden = |path: &Path| hidden.iter().any(|secret| path.starts_with(secret));
    for path in spec
        .write
        .iter()
        .filter(|path| path.exists() && !inside_hidden(path))
    {
        same(&mut args, "--bind", path);
    }
    let mut dirs = Vec::new();
    for path in &hidden {
        if path.is_dir() {
            args.extend([OsString::from("--tmpfs"), path.as_os_str().to_owned()]);
            dirs.push(path);
        } else {
            args.extend([
                OsString::from("--ro-bind"),
                OsString::from("/dev/null"),
                path.as_os_str().to_owned(),
            ]);
        }
    }
    for path in spec
        .write
        .iter()
        .filter(|path| path.exists() && inside_hidden(path))
    {
        same(&mut args, "--bind", path);
    }
    for path in spec.reveal.iter().filter(|path| path.exists()) {
        same(&mut args, "--ro-bind", path);
    }
    // 脚本调工具的通道（施工 M-3）：落在藏起来的里面的，挂回来才连得上。
    for path in spec
        .sockets
        .iter()
        .filter(|path| path.exists() && inside_hidden(path))
    {
        same(&mut args, "--bind", path);
    }
    for path in dirs {
        args.extend([OsString::from("--remount-ro"), path.as_os_str().to_owned()]);
    }
    args
}

/// 一条挂载，来处和去处是同一个 `path`。
fn same(args: &mut Vec<OsString>, flag: &str, path: &Path) {
    args.push(OsString::from(flag));
    args.push(path.as_os_str().to_owned());
    args.push(path.as_os_str().to_owned());
}

/// 里面那一次的规格：藏起来的、放开只读的挂载已经办了，只留能写的给 Landlock（拦系统服务的套接字、能写的以外写不了），和
/// 要连的套接字（施工 M-3：Landlock 照样只放行它们）。
fn inner(spec: &Spec) -> Spec {
    Spec {
        write: spec.write.clone(),
        hidden: Vec::new(),
        read: None,
        reveal: Vec::new(),
        mount: false,
        sockets: spec.sockets.clone(),
    }
}

/// 换成 bwrap：挂好以后在里面再跑一次助手（[`inner`] 的规格），再换成 `program`。没有能用的 bwrap 的，说一句、退出 125、
/// 不跑：规格要挂载，不能悄悄退回 Landlock、私密目录对命令敞开。
pub(super) fn exec(spec: &Spec, program: &OsStr, args: &[OsString]) -> ExitCode {
    let Some(bwrap) = found() else {
        return crate::fail("cannot confine: bubblewrap is not available");
    };
    let helper = match std::env::current_exe() {
        Ok(helper) => helper,
        Err(error) => return crate::fail(&format!("cannot confine: {error}")),
    };
    let inner = match inner(spec).to_json() {
        Ok(inner) => inner,
        Err(error) => return crate::fail(&format!("cannot confine: {error}")),
    };
    // 助手自己被藏起来的盖住了（私密目录包住了装的位置、工作树）：只读挂回来，bwrap 才在里面找得到它（施工 5-15 下再修）。
    let mut outer = spec.clone();
    if spec.hides(&helper) {
        outer.reveal.push(helper.clone());
    }
    let mut command: Vec<OsString> = mounts(&outer);
    command.push(OsString::from("--"));
    command.push(helper.into_os_string());
    command.extend(["run", "--spec"].map(OsString::from));
    command.push(OsString::from(inner));
    command.push(OsString::from("--"));
    command.push(program.to_owned());
    command.extend(args.iter().cloned());
    crate::unix::exec(bwrap.as_os_str(), &command)
}

/// 这台机器上能不能用（`probe` 时问一次）：找得到可信的 bwrap，照正式的挂载在临时目录里真起一次，里面藏起来的是空的、写进去
/// 报错，能写的写得进，都对才算。
pub(super) fn usable() -> bool {
    let Some(bwrap) = found() else {
        return false;
    };
    let base = std::env::temp_dir().join(format!("miyu-sandbox-bwrap-{}", std::process::id()));
    let ok = try_once(&bwrap, &base).unwrap_or(false);
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响探测"
    )]
    let _ = fs::remove_dir_all(&base);
    ok
}

/// [`usable`] 的那一次：能写的 `w`、它里面藏起来的 `w/h`（放一份 `secret`）。
fn try_once(bwrap: &Path, base: &Path) -> std::io::Result<bool> {
    let writable = base.join("w");
    let hidden = writable.join("h");
    fs::create_dir_all(&hidden)?;
    fs::write(hidden.join("secret"), "x")?;
    let (writable, hidden) = (fs::canonicalize(writable)?, fs::canonicalize(hidden)?);
    let spec = Spec {
        write: vec![writable.clone()],
        hidden: vec![hidden.clone()],
        read: None,
        reveal: Vec::new(),
        mount: true,
        sockets: Vec::new(),
    };
    let script = format!(
        "test ! -e '{h}/secret' && ! touch '{h}/x' 2>/dev/null && touch '{w}/ok'",
        h = hidden.display(),
        w = writable.display()
    );
    let mut args = mounts(&spec);
    args.extend(["--", "/bin/sh", "-c"].map(OsString::from));
    args.push(OsString::from(script));
    let status = Command::new(bwrap)
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    Ok(status.success() && writable.join("ok").exists() && !hidden.join("x").exists())
}

#[cfg(test)]
mod tests;
