//! 沙盒数据根和拉起的核心（施工 V-1）：照 `cargo xtask dev-home` 的写法建骨架、写一份系统配置，模型指到假服务。
//!
//! 环境照集成测试拉核心的那一套（`crates/miyu/tests/support`）：不去 models.dev 拉目录，缓存目录放进数据根，不用
//! `$XDG_RUNTIME_DIR`，不走代理。模型的窗口写成 [`WINDOW`]（施工 V-2 上）：长会话照真用时那样到线就压，量的是压过的会话；
//! V-1 没写窗口，会话一直不压，大会话的投影是把一万条事件整份组装，真用时到不了。

use std::ffi::OsString;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// 量尺的模型的上下文窗口（施工 V-2 上）：常见的 20 万，压缩线 17 万。
pub const WINDOW: u64 = 200_000;

use miyu_ipc::Lock;
use miyu_store::env::{Env, Platform};
use miyu_store::root::DataRoot;

/// 核心起来最多等多久。
const START_PATIENCE: Duration = Duration::from_secs(60);

/// 在模拟器里起来最多等多久（施工 LW-1，[`Traced`]）：模拟器头一回执行每一块代码都要先翻译、写进日志，核心起来比原生慢十几
/// 倍，CI 的机器再慢几倍；只在排函数那一场用，别的几场照 [`START_PATIENCE`]。带页探针起的（[`Probed`]）也照
/// [`START_PATIENCE`]：程序以接近原生的速度跑。
const TRACED_PATIENCE: Duration = Duration::from_secs(600);

/// 一个沙盒数据根，和拉起核心要的几样。
pub struct Sandbox {
    /// 数据根。
    pub root: DataRoot,
    /// 会话在哪个目录干活：数据根旁边的 `work/`。
    pub work: PathBuf,
    miyu: PathBuf,
    resources: PathBuf,
}

impl Sandbox {
    /// 在 `dir` 下建一个新的：有旧的先删掉。模型是 `base_url` 上的假服务。
    ///
    /// # Errors
    ///
    /// 删不掉旧的、建不了、写不进配置。
    pub fn new(
        dir: &Path,
        miyu: &Path,
        resources: &Path,
        base_url: &str,
    ) -> Result<Sandbox, String> {
        if dir.exists() {
            std::fs::remove_dir_all(dir)
                .map_err(|e| format!("删不掉旧的 {}：{e}", dir.display()))?;
        }
        let home = dir.join("home");
        let work = dir.join("work");
        std::fs::create_dir_all(&work).map_err(|e| format!("建不了 {}：{e}", work.display()))?;
        let root = DataRoot::locate(&Env {
            platform: Platform::current(),
            miyu_home: Some(home.into_os_string()),
            home: None,
            xdg_cache_home: None,
            local_app_data: None,
            miyu_resources: None,
            exe: None,
        })
        .map_err(|e| format!("数据根找不到：{e}"))?;
        root.prepare().map_err(|e| format!("数据根建不了：{e}"))?;
        let config = format!(
            "[providers.perf]\ndriver = \"openai-chat\"\nbase_url = \"{base_url}\"\nkey = {{ env = \"MIYU_PERF_KEY\" }}\n\n[providers.perf.models.perf-model]\nwindow = {WINDOW}\n\n[models]\nchat = \"perf/perf-model\"\n"
        );
        std::fs::write(root.config_file(), config).map_err(|e| format!("系统配置写不进：{e}"))?;
        Ok(Sandbox {
            root,
            work,
            miyu: miyu.to_path_buf(),
            resources: resources.to_path_buf(),
        })
    }

    /// 拉起核心，等它写来 `ready`：交回跑着的核心，和从拉起到 `ready` 用了多久。
    ///
    /// # Errors
    ///
    /// 拉不起来、没写 `ready` 就走了、等太久。
    pub fn start(&self) -> Result<Running, String> {
        self.start_with(Launch::default())
    }

    /// 照 `launch` 拉起核心（施工 V-29）：放不放进自己的 systemd scope、运行日志记到哪一级；在不在模拟器里、信任哪份证书
    /// （施工 LW-1）。别的同 [`Sandbox::start`]。
    ///
    /// # Errors
    ///
    /// 拉不起来、没写 `ready` 就走了、等太久。
    pub fn start_with(&self, launch: Launch<'_>) -> Result<Running, String> {
        let cache = self.root.perf_cache();
        let began = Instant::now();
        // 在模拟器里的（施工 LW-1）：模拟器拉起核心，进程号是模拟器的，`/proc` 里读到的映射就是核心的；不放进 scope。
        // 放进 scope 的经 `systemd-run --scope`：它登记好 scope 就把自己换成核心（`env` 也是换，进程号不变），环境照传。
        // `XDG_RUNTIME_DIR` 留给 systemd-run 找用户的总线，进了 scope 再由 `env` 去掉。带页探针的（施工 MO-2）不换程序，
        // 只多几个环境变量（[`Probed::vars`]），也不放进 scope；`traced`、`probed` 都给了的照 `traced`。
        let mut command = if let Some(traced) = launch.traced {
            let mut command = Command::new(traced.emulator);
            command
                .args(["-d", "in_asm", "-D"])
                .arg(traced.log)
                .arg(&self.miyu)
                .env_remove("XDG_RUNTIME_DIR");
            command
        } else if let Some(probed) = launch.probed {
            let mut command = Command::new(&self.miyu);
            command.envs(probed.vars()?).env_remove("XDG_RUNTIME_DIR");
            command
        } else if launch.scoped {
            if !cfg!(target_os = "linux") {
                return Err("放进自己的 systemd scope 只在 Linux 上做".to_string());
            }
            let mut command = Command::new("systemd-run");
            command
                .args(["--user", "--scope", "--quiet", "--collect", "--"])
                .args(["env", "-u", "XDG_RUNTIME_DIR"])
                .arg(&self.miyu);
            command
        } else {
            let mut command = Command::new(&self.miyu);
            command.env_remove("XDG_RUNTIME_DIR");
            command
        };
        command.env_remove("MIYU_LOG");
        // 照 `miyu` 拉起核心的样子带上线程缓存少留的那一项（施工 LW-12）：量的是人用到的那个样子；模拟器里录的时候核心也就
        // 不会为了补上它重新执行自己、跑出模拟器。
        command.envs(miyu_heap::launch::vars(
            std::env::var_os(miyu_heap::launch::TUNABLES).as_deref(),
        ));
        if let Some(level) = launch.log {
            command.env("MIYU_LOG", level);
        }
        if let Some(trust) = launch.trust {
            command.env("SSL_CERT_FILE", trust);
        }
        if let Some(home) = launch.home {
            command.envs(home_vars(home));
        }
        command
            .env_remove("HTTP_PROXY")
            .env_remove("HTTPS_PROXY")
            .env_remove("ALL_PROXY");
        if let Some(proxy) = launch.proxy {
            command.env("HTTP_PROXY", proxy);
        }
        let mut child = command
            .args(["core", "--idle-seconds", "3600"])
            .env("MIYU_HOME", self.root.path())
            .env("MIYU_RESOURCES", &self.resources)
            .env("MIYU_CATALOG_UPDATE", "false")
            .env("XDG_CACHE_HOME", &cache)
            .env("LOCALAPPDATA", &cache)
            .env("MIYU_PERF_KEY", "perf")
            .env("NO_PROXY", "127.0.0.1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| match launch.traced {
                Some(traced) => format!("拉不起模拟器 {}：{e}", traced.emulator.display()),
                None => format!("拉不起 {}：{e}", self.miyu.display()),
            })?;
        let stdout = child.stdout.take().ok_or("核心的标准输出没接上")?;
        // 读那一行放到别的线程：核心卡住不写也不走的时候，这里照样能按时放弃。
        let (sender, line) = mpsc::channel();
        std::thread::spawn(move || {
            let mut text = String::new();
            let read = BufReader::new(stdout).read_line(&mut text).map(|_| text);
            #[expect(clippy::let_underscore_must_use, reason = "等的一方放弃了就没人收")]
            let _ = sender.send(read);
        });
        let mut running = Running {
            child,
            ready: Duration::ZERO,
        };
        let patience = match launch.traced {
            Some(_) => TRACED_PATIENCE,
            None => START_PATIENCE,
        };
        match line.recv_timeout(patience) {
            Ok(Ok(text)) if text == "ready\n" => {
                running.ready = began.elapsed();
                Ok(running)
            }
            outcome => {
                running.kill();
                Err(format!(
                    "核心没起来（{outcome:?}）：{}",
                    tail(&self.root.logs().join("core.log"))
                ))
            }
        }
    }

    /// 在沙盒里跑一次命令行 `miyu <words>`（施工 LW-30，重场景里的 `miyu kb`、`miyu ask` 这些）：数据根、资源、缓存同核心，
    /// 家目录和跟着它的几个目录指到 `home`（[`home_vars`]），不走代理；它连上正在运行的核心。交回标准输出。
    ///
    /// # Errors
    ///
    /// 拉不起来、退出码不是 0（报错里带它的标准错误）。
    #[cfg(unix)]
    pub fn cli(&self, words: &[&str], home: &Path) -> Result<String, String> {
        let output = Command::new(&self.miyu)
            .args(words)
            .envs(home_vars(home))
            .env("MIYU_HOME", self.root.path())
            .env("MIYU_RESOURCES", &self.resources)
            .env("MIYU_CATALOG_UPDATE", "false")
            .env("XDG_CACHE_HOME", self.root.perf_cache())
            .env("NO_PROXY", "127.0.0.1")
            .env_remove("XDG_RUNTIME_DIR")
            .env_remove("MIYU_LOG")
            .env_remove("HTTP_PROXY")
            .env_remove("HTTPS_PROXY")
            .env_remove("ALL_PROXY")
            .stdin(Stdio::null())
            .output()
            .map_err(|e| format!("拉不起 {}：{e}", self.miyu.display()))?;
        if !output.status.success() {
            return Err(format!(
                "miyu {} 退出了（{}）：{}",
                words.join(" "),
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    /// 停掉核心，等它放开数据根的锁：下一次拉起量的才是干净的冷启动。
    ///
    /// # Errors
    ///
    /// 等了一分钟锁还在。
    pub fn stop(&self, mut running: Running) -> Result<(), String> {
        running.kill();
        let deadline = Instant::now() + START_PATIENCE;
        while Instant::now() < deadline {
            if Lock::acquire(&self.root).is_ok() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Err("核心停了，数据根的锁一分钟还没放开".to_string())
    }
}

/// 怎么拉起核心（施工 V-29）。默认的照别的几场：不放进 scope，不记运行日志的级别（核心照配置，出厂 `info`）。
#[derive(Debug, Clone, Copy, Default)]
pub struct Launch<'a> {
    /// 放进自己的 systemd scope：量连子进程的 CPU 要它（`cpu.rs`）。要 systemd 的用户会话，只在 Linux 上做，别的系统上
    /// 给了的 [`Sandbox::start_with`] 报错。
    pub scoped: bool,
    /// `MIYU_LOG`：拆开一步各段时记到 `debug`，核心每处理一条输入、每做一个动作记一行（`log.md`）。
    pub log: Option<&'a str>,
    /// 放进模拟器里起（施工 LW-1，排函数）：给了的不看 [`Launch::scoped`]、[`Launch::probed`]。
    pub traced: Option<Traced<'a>>,
    /// 带页探针起（施工 MO-2，排函数，macOS）：和 [`Launch::traced`] 并列的另一种录法，给了的不看 [`Launch::scoped`]。
    pub probed: Option<Probed<'a>>,
    /// 核心的 `SSL_CERT_FILE`（施工 LW-1）：假模型 HTTPS 的口要核心信任测试证书；没给的不设，照宿主的环境。
    pub trust: Option<&'a Path>,
    /// 家目录指进沙盒（施工 LW-6，[`home_vars`]）：配着真的终端界面录的两段，核心和终端界面看到同一个家目录；没给的照宿主的。
    pub home: Option<&'a Path>,
    /// 核心的 `HTTP_PROXY`（施工 LW-30）：重场景里网页从量尺的假代理抓（地址闸不放回环，本机解析不出的名字交给代理）；
    /// 回环上的假模型、假服务照 `NO_PROXY` 直连。没给的不走代理。
    pub proxy: Option<&'a str>,
}

/// 家目录 `home` 和跟着它的几个目录（施工 LW-6）：`HOME`、`XDG_CONFIG_HOME`、`XDG_DATA_HOME`、`XDG_STATE_HOME`，
/// 系统加装的软件包只看 `MIYU_DISTRO_PACKAGES`（一个空目录，宿主装过的 Miyu 软件包不混进来）。`XDG_CACHE_HOME` 不在里面：
/// 核心的照 [`Sandbox::start_with`] 放进数据根，终端界面的由调用方另给。`MIYU_DISTRO_PACKAGES` 要先建好。
///
/// `TMPDIR` 不指进沙盒：数据根的路径长了，核心的套接字放不进数据根的 `run/`，要退到临时目录下（`miyu-ipc` 的 `place.rs`），
/// 临时目录也在沙盒里的话照样超过套接字路径的上限，核心起不来（工作树的路径带中文时实测撞上过）。别的几场也是照宿主的。
pub fn home_vars(home: &Path) -> [(&'static str, PathBuf); 5] {
    [
        ("HOME", home.to_path_buf()),
        ("XDG_CONFIG_HOME", home.join(".config")),
        ("XDG_DATA_HOME", home.join(".local").join("share")),
        ("XDG_STATE_HOME", home.join(".local").join("state")),
        ("MIYU_DISTRO_PACKAGES", home.join("distro")),
    ]
}

/// 在模拟器里起核心（施工 LW-1）：`<emulator> -d in_asm -D <log> <miyu> core …`。QEMU 的用户态模拟器头一回要执行一块代码时
/// 先翻译它，`in_asm` 把这一块每条指令的地址写进日志：日志就是执行过的代码，先后是头一回执行的先后。
#[derive(Debug, Clone, Copy)]
pub struct Traced<'a> {
    /// 模拟器，例如 `qemu-x86_64`。
    pub emulator: &'a Path,
    /// 这一次的日志写到哪。
    pub log: &'a Path,
}

/// 页探针重新关上代码页的间隔（毫秒，施工 MO-2，探针的 `MIYU_PAGES_REARM_MS`）。来由：MO-1 在一台 M1 MacBook 上量过，
/// 每 20 毫秒重关一轮，一轮记到的函数是不重关时的 2.7 倍，用时看不出变慢。
pub const REARM_MS: u32 = 20;

/// 带页探针起被测程序（施工 MO-2，macOS 上代替 [`Traced`]）：不换程序，只给进程加几个环境变量（[`Probed::vars`]）。探针
/// （`crates/miyu-perf/probe/pagetrace.c`）由 `DYLD_INSERT_LIBRARIES` 插进去，只在可执行文件是 [`Probed::program`] 的进程里
/// 动手，每个进程写 `<out>.<进程号>` 和旁边的 `<out>.<进程号>.exe`。它写下的已经是符号表里的地址，所以这一路用不着装载起点。
#[derive(Debug, Clone, Copy)]
pub struct Probed<'a> {
    /// 探针的动态库。
    pub library: &'a Path,
    /// 这一段的输出前缀（探针的 `MIYU_PAGES_OUT`）。
    pub out: &'a Path,
    /// 被测程序（探针的 `MIYU_PAGES_PROGRAM`）：别的进程载入了探针也什么都不做，例如核心起的沙盒子进程、换成终端界面以前的
    /// 主程序。
    pub program: &'a Path,
}

impl Probed<'_> {
    /// 交给被测进程的环境变量：`DYLD_INSERT_LIBRARIES`、`MIYU_PAGES_OUT`、`MIYU_PAGES_PROGRAM`、`MIYU_PAGES_REARM_MS`
    /// （[`REARM_MS`]）。三个路径都换成绝对路径：被测进程的工作目录不是量尺的（终端界面在工具一步的工作区里起），探针照
    /// 相对路径找会找错地方。
    ///
    /// # Errors
    ///
    /// 探针、被测程序找不到；输出前缀换不成绝对路径。
    pub fn vars(&self) -> Result<[(&'static str, OsString); 4], String> {
        let library = self
            .library
            .canonicalize()
            .map_err(|e| format!("找不到探针 {}：{e}", self.library.display()))?;
        let program = self
            .program
            .canonicalize()
            .map_err(|e| format!("找不到 {}：{e}", self.program.display()))?;
        let out = std::path::absolute(self.out)
            .map_err(|e| format!("探针的输出前缀 {} 换不成绝对路径：{e}", self.out.display()))?;
        Ok([
            ("DYLD_INSERT_LIBRARIES", library.into_os_string()),
            ("MIYU_PAGES_OUT", out.into_os_string()),
            ("MIYU_PAGES_PROGRAM", program.into_os_string()),
            ("MIYU_PAGES_REARM_MS", OsString::from(REARM_MS.to_string())),
        ])
    }
}

/// 跑着的核心。丢掉时结束它。
pub struct Running {
    child: Child,
    /// 从拉起到写来 `ready` 用了多久。
    pub ready: Duration,
}

impl Running {
    /// 核心的进程号；在模拟器里的是模拟器的（施工 LW-1）。
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// 程序 `program` 装在进程的哪个地址起（施工 LW-1）：读 `/proc/<进程号>/maps`。在模拟器里的，日志里的地址减去它就是
    /// 符号表里的地址。只在模拟器那一路用：带页探针的（[`Probed`]）用不着，macOS 上也没有 `/proc`。
    ///
    /// # Errors
    ///
    /// 找不到程序（路径不对）、映射读不了（不是 Linux）、映射里没有它。
    pub fn base(&self, program: &Path) -> Result<u64, String> {
        base_of(self.pid(), program)
    }

    fn kill(&mut self) {
        #[expect(clippy::let_underscore_must_use, reason = "已经走了的结束不了，照样收")]
        let _ = self.child.kill();
        #[expect(
            clippy::let_underscore_must_use,
            reason = "收不到也只是留一个僵尸到量尺退出"
        )]
        let _ = self.child.wait();
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        self.kill();
    }
}

/// 照 `/proc/<进程号>/maps` 的原文 `maps` 找程序 `program` 的装载起点：路径是它、文件偏移是 0 的那一段的起点。一行是
/// `<起>-<止> <权限> <偏移> <设备> <节点> <路径>`，路径里可以有空格，读到行尾。没有的是 `None`。
pub fn load_base(maps: &str, program: &Path) -> Option<u64> {
    maps.lines().find_map(|line| {
        let mut rest = line;
        let mut fields = [""; 5];
        for field in &mut fields {
            let trimmed = rest.trim_start();
            let end = trimmed.find(' ')?;
            *field = &trimmed[..end];
            rest = &trimmed[end..];
        }
        let [range, _, offset, _, _] = fields;
        if u64::from_str_radix(offset, 16).ok()? != 0 || Path::new(rest.trim_start()) != program {
            return None;
        }
        u64::from_str_radix(range.split_once('-')?.0, 16).ok()
    })
}

/// 进程 `pid` 里程序 `program` 装在哪个地址起（施工 LW-1，[`Running::base`]；施工 LW-6 也给伪终端里在模拟器中跑的终端界面用）：
/// 读 `/proc/<进程号>/maps`。
///
/// # Errors
///
/// 找不到程序（路径不对）、映射读不了（不是 Linux）、映射里没有它。
pub fn base_of(pid: u32, program: &Path) -> Result<u64, String> {
    let program = program
        .canonicalize()
        .map_err(|e| format!("找不到 {}：{e}", program.display()))?;
    let maps = format!("/proc/{pid}/maps");
    let text = std::fs::read_to_string(&maps).map_err(|e| format!("读不了 {maps}：{e}"))?;
    load_base(&text, &program).ok_or_else(|| format!("{maps} 里没有 {} 的映射", program.display()))
}

/// 运行日志的最后几行：核心起不来时附在报错后面。
fn tail(log: &Path) -> String {
    let text = std::fs::read_to_string(log).unwrap_or_default();
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(20)..].join("\n")
}

#[cfg(test)]
mod tests;
