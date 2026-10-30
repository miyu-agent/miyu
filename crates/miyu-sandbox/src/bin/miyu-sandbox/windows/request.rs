//! 规格翻成 MXC 的请求（施工 5-9 改上）：能写的进读写路径，藏起来的进禁止路径（禁止压过只读、读写，2026-10-21 虚拟机上量过；
//! 所以藏起来的里面有能写的、单独放开只读的，改禁它们旁边的，施工 SW-6，`miyu_sandbox::carve`；正被别的进程独占着的文件
//! 不交，施工 SW-1，`miyu_sandbox::deniable`），
//! 读照设计 11 A4 整盘放开：每个固定盘的盘根进只读路径，单独放开只读的（技能文件夹）也进；网络照设计 11 A5 不管，进出都放行；
//! 窗口放开（全关会挡 Win32k，PowerShell、git 起不来），剪贴板、模拟输入照 MXC 的默认不给。环境照助手自己的原样交（执行器
//! 已经拣好了），不叠 MXC 的默认；工作目录是助手的。不限时：时限由执行器管。

use std::ffi::{OsStr, OsString};
use std::path::Path;

use mxc_sdk::v1::{
    ContainerRequest, Containment, FilesystemPolicy, NetworkAction, NetworkEgressPolicy,
    NetworkIngressPolicy, NetworkPolicy, UiPolicy,
};

use miyu_sandbox::Spec;

/// 照规格 `spec`、程序 `program`、参数 `args`、这台机器的固定盘的盘根 `drives` 拼请求。命令行、工作目录、环境变量不是合法的
/// Unicode 的拼不成，说为什么。
pub(super) fn of(
    spec: &Spec,
    program: &OsStr,
    args: &[OsString],
    drives: Vec<String>,
) -> Result<ContainerRequest, String> {
    let command = miyu_sandbox::windows_command_line(program, args)
        .ok_or("the command line is not valid Unicode")?;
    let mut request = ContainerRequest::new(command);
    request.containment = Containment::Process;
    request.working_directory = std::env::current_dir()
        .ok()
        .and_then(|dir| dir.to_str().map(str::to_string));
    let text = |path: &Path| path.to_string_lossy().into_owned();
    request.filesystem = Some(FilesystemPolicy {
        readwrite_paths: spec.write.iter().map(|path| text(path)).collect(),
        readonly_paths: drives
            .into_iter()
            .chain(spec.reveal.iter().map(|path| text(path)))
            .collect(),
        denied_paths: miyu_sandbox::deniable(&carved(spec), miyu_sandbox::busy)
            .iter()
            .map(|path| text(path))
            .collect(),
        ..Default::default()
    });
    request.network = Some(NetworkPolicy {
        egress: Some(NetworkEgressPolicy {
            default: Some(NetworkAction::Allow),
            ..Default::default()
        }),
        ingress: Some(NetworkIngressPolicy {
            default: Some(NetworkAction::Allow),
            ..Default::default()
        }),
        ..Default::default()
    });
    request.ui = Some(UiPolicy {
        disable: false,
        ..Default::default()
    });
    let environment: Option<Vec<(String, String)>> = std::env::vars_os()
        .map(|(key, value)| Some((key.into_string().ok()?, value.into_string().ok()?)))
        .collect();
    request.environment = Some(environment.ok_or("an environment variable is not valid Unicode")?);
    request.inherit_default_environment = Some(false);
    Ok(request)
}

/// 规格里藏起来的，里面能写的、单独放开只读的挖出来（施工 SW-6）。
fn carved(spec: &Spec) -> Vec<std::path::PathBuf> {
    let open: Vec<_> = spec.write.iter().chain(&spec.reveal).cloned().collect();
    miyu_sandbox::carve(&spec.hidden, &open, miyu_sandbox::entries)
}
