//! Windows 的命名管道只对本人开放（`docs/designs/04-核心协议.md` 第二节「各平台的坑」，施工 3-8 补）。
//!
//! 命名管道默认的安全描述符允许 Everyone 读，要显式设成只允许当前用户；头连上以后，还要核对管道另一头
//! 的进程是自己的。这几样要调 Windows 的安全接口，整个仓库只有这个 crate 放开了 `unsafe`（2026-09-27
//! 项目主人定：单开一个小 crate），而且只装非用它不可的三样：
//!
//! - `create`：建管道的一个实例，只对当前用户开放，可以声明是第一个实例；
//! - `create_for_containers`：同上，另让当前用户沙盒里的命令连得上（施工 M-3 下，脚本调工具的通道）；
//! - `client_process`：管道另一头（客户端）进程的编号；
//! - `current_user`：当前用户的 SID；
//! - `server_user`：管道另一头（服务端）进程的用户 SID。
//!
//! 等连接、管道忙了再连这些用不着 `unsafe` 的，在 `miyu-ipc` 里。别的平台上这个 crate 只有
//! [`owner_only`]。

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use windows::{client_process, create, create_for_containers, current_user, server_user};

/// 只允许 `sid` 这个用户的安全描述符，写成 SDDL：`D:` 是访问控制列表，`P` 是受保护、不继承上一层的；
/// 只有一条：允许（`A`）全部权限（`GA`）给他。
pub fn owner_only(sid: &str) -> String {
    format!("D:P(A;;GA;;;{sid})")
}

/// 沙盒里的命令连管道要的权限，只够连上、读写（施工 M-3 下）：读数据、写数据、读属性、同步（`CreateFileW` 总会带上后两样）。
/// 不含追加数据：在管道上它是「建实例」，给了容器就能抢着建同名的实例。客户端照这个开，不照 `GENERIC_READ | GENERIC_WRITE`：
/// 那会多要几样，容器里的过不了。
pub const CONNECT_ACCESS: u32 = 0x0010_0083;

/// 当前用户 `sid` 全部权限，沙盒里的命令（Windows 的基础容器、AppContainer，令牌带 `ALL APPLICATION PACKAGES`）只给
/// [`CONNECT_ACCESS`]。容器里的进程照访问检查要同时过用户那一条和包那一条，所以别的用户的容器照旧进不来。完整性标签不用动：
/// 基础容器的令牌不是低完整性的（虚拟机里量过，放到低和不放一样连得上）。
pub fn reachable_from_containers(sid: &str) -> String {
    format!("D:P(A;;GA;;;{sid})(A;;0x{CONNECT_ACCESS:x};;;AC)")
}

#[cfg(test)]
mod tests;
