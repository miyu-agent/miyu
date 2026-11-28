## 施工单 CRT-1：Windows 的程序不再要 VC++ 运行库

状态：做完，等验收（分支 `step/CRT-1-静态C运行库`）。2026-11-28 core1 派：优化会话读 rc.5 的 Windows 发行包查出来的。

### 目的

没装过 VC++ 2015–2022 运行库的 Windows 机器上，`miyu.exe`、`miyu-tui.exe`、`miyu-web.exe` 和核心的两个小程序照样起得来。

### 经过

1. rc.5 的 `miyu-2026.11.0-rc.5-windows-x86_64.zip` 里五个 `.exe` 的导入表都有 `VCRUNTIME140.dll` 和一组 `api-ms-win-crt-*`（`objdump -p` 读的）。`api-ms-win-crt-*` 是通用 C 运行库，Windows 10 起系统自带；`VCRUNTIME140.dll` 是装 VC++ 运行库时放进系统的，没装过的机器上报「找不到 VCRUNTIME140.dll」、起不来。install-test 在 GitHub 的 Windows 机器上能过，是因为那台机器本来装着运行库。
2. 主会话定：MSVC 目标一律加 `-C target-feature=+crt-static`，写在仓库的 `.cargo/config.toml`。
3. 两个头在 `<target>/dist/heads/<编号>/` 里各自编，那边的 `.cargo/config.toml` 只有 `xtask` 的别名。cargo 从检出往上找配置，默认的位置在核心仓库里面，碰巧也读得到核心这一份；`--out` 换到仓库外面就读不到了。所以发版那台另用只管 MSVC 目标的环境变量 `CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS` 给每一次 cargo 加上（技术题，按推荐定）：不用 `RUSTFLAGS`，因为它会盖掉各个仓库配置里写的全部 rustflags。
4. 防回退放在查包：`cargo xtask dist-check` 解开 Windows 的包以后读每个 `.exe` 的导入表，要了 `VCRUNTIME`、`MSVCP` 开头的 DLL 就报、这一趟不发。导入表照 PE 格式自己读，不靠机器上的 dumpbin（要找 VS 环境）、llvm-readobj（机器镜像不保证有）：同一份代码哪台机器上都一样，Linux 上也测得到。

### 照的图纸

- 设计 `12-进程形态与分发.md` 第六节平台表的 Windows 一行；`docs/{zh,en}/dev/contributing.md` 的 `cargo xtask dist-check` 一行。

### 改了什么

- `.cargo/config.toml`：`[target.x86_64-pc-windows-msvc] rustflags = ["-C", "target-feature=+crt-static"]`。cc 编的 C 代码（SQLite、zstd、ring）看到这个特性也换成静态的运行库（`/MT`）。不发 `aarch64-pc-windows-msvc`，不写它。
- `.github/workflows/release.yml`：`dist` 一项加 `CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS: -C target-feature=+crt-static`，别的三台不编 MSVC 目标，不起作用。
- `xtask/src/dist/check/crt.rs`：读导入表、认 VC++ 运行库；`check.rs` 的 `contents` 对 Windows 的包每个程序查一次。

### 怎么验

- `xtask/src/dist/check/crt/tests.rs`：现搭的 PE 文件读得出导入表、截断的报错，`VCRUNTIME140.dll`、`vcruntime140_1.dll`、`MSVCP140.dll` 算、`msvcrt.dll`、`api-ms-win-crt-*`、`ucrtbase.dll` 不算，Windows 的包五个程序都查、别的系统不查；MSVC 的 Windows 上读这个测试程序自己的导入表，不许有 VC++ 运行库（不加 `+crt-static` 时红）。
- GitHub 的 Windows 机器上照 `release.yml` 打一次包，`dumpbin -dependents` 读改前、改后的五个程序（临时的 `measure/crt-1-*` 分支，不进 main）：结果和大小见下。

  2026-11-28 跑的两趟：改前（不加 `+crt-static`、带查包的新一项）五个程序都要 `VCRUNTIME140.dll`，查包报 5 处、这一趟不发，读测试程序自己的那一条红（它也要 `VCRUNTIME140.dll`）；改后五个程序都只剩系统的 DLL，`api-ms-win-crt-*` 也一起没了，查包没有问题，那一条绿。

  | 程序 | 改前（字节） | 改后（字节） | 多了 |
  |---|---|---|---|
  | `miyu.exe` | 43,052,032 | 43,230,208 | 178,176（0.41%） |
  | `miyu-sandbox.exe` | 1,678,848 | 1,776,128 | 97,280（5.79%） |
  | `miyu-extract.exe` | 2,398,720 | 2,496,000 | 97,280（4.06%） |
  | `miyu-tui.exe` | 20,225,024 | 20,403,712 | 178,688（0.88%） |
  | `miyu-web.exe` | 8,232,448 | 8,394,752 | 162,304（1.97%） |
  | 合计 | 75,587,072 | 76,300,800 | 713,728（0.94%） |
  | 压缩包 `.zip` | 35,570,413 | 35,958,955 | 388,542（1.09%） |

### 不做什么

- 托盘（`miyu-applet`）、im、扩展包在别的仓库，各自的发版要照这里加同一条；这一步只写进汇报。
