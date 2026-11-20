## 施工单 MB-1：成员沙盒补强

状态：核心的分会话做。2026-10-24 项目主人定了形状（主会话转）：不做「默认不能、组里开」的开关；成员能跑命令，但只在他自己的沙盒里；要出沙盒的，管理员照现在问人，成员直接拒、不问；沙盒外的读成员一律拒，只留程序跑起来必需的系统目录只读；QQ 群、陌生人私聊照旧在场所自己的工作区沙盒里能跑。所以 MB-1 是「核实并补强成员沙盒」。设计 06 U5 那条主会话改。

### 核实到的

- 出沙盒：成员、场所会话要出沙盒的（`outside_sandbox`、管理员放行过的）一律回 `no-commands`、不问人（`guard.rs`，Y-2 下①）。对。
- 读：关起来的会话的规格写 `read` 白名单（`sandbox/confined.rs`），数据根整个藏着（别的成员的家目录、机主的数据都在里面），跑核心的用户的家目录不在名单里。对。但系统目录整个放：Linux 是 `/usr /bin /sbin /lib /lib32 /lib64 /etc /opt`，macOS 是 `/usr /bin /sbin /System /Library /Applications /opt /etc`，比「只留必需的」宽。
- Windows：读关不住（`CONFINES_READS` 假），成员和场所会话一律不跑命令。等于「沙盒用不了不让跑」。
- 漏洞：Linux 上连 Unix 套接字（Landlock 第 9 版的 `ResolveUnix`）、抽象套接字（第 6 版的范围）是「内核有就管」。没有的内核（眼下大多数发行版、CI 的 ubuntu-24.04）上，成员、群会话的命令连得上机主的 D-Bus 会话总线（能让 `systemd --user` 替它在沙盒外跑东西）、Docker、ssh-agent，等于出得了沙盒。

### 做了什么

1. 读名单收窄（成员和场所会话共用这一份，`sandbox/confined.rs` 的 `system_reads`）：
   - Linux：`/usr /bin /sbin /lib /lib32 /lib64`；`/etc` 只给证书（`ssl`、`ca-certificates`、`pki`、`crypto-policies`）、域名解析（`resolv.conf`、`hosts`、`nsswitch.conf`、`host.conf`、`gai.conf`）、`ld.so.cache` 和 `ld.so.conf(.d)`、`localtime`、`timezone`、`passwd`、`group`、`services`、`protocols`、`alternatives`、`mtab`、`terminfo`、`inputrc`、`gitconfig`、`mime.types`，和 Debian 系 Python 的 `/etc/python*`；去掉 `/opt`。
   - macOS：`/usr /bin /sbin /System`、`/Library/Developer/CommandLineTools`、`/Library/Apple`、`/Library/Frameworks`、`/opt/homebrew`、`xcode-select` 选的开发者目录（照 `/private/var/db/xcode_select_link` 换成真实位置）、程序起来要读的那三处；`/etc` 同上那几样；去掉 `/Applications`、`/Library` 别处、别的 `/opt`（主会话定 C：只留 `/opt/homebrew`）。
   - `passwd`、`group` 放不掉：不放的话 `whoami` 报找不到名字、Python 的 `pwd.getpwuid` 报 `KeyError`（nss 的 systemd 那一路要连 userdb 的套接字，沙盒里连不上，靠 nsswitch 绕不过去）；`git`、`curl`、`getpass.getuser` 照常。照放，记进 SEC-2。
2. Linux 的套接字（主会话定 B：选 seccomp）：内核的 Landlock 两样都管得住的照它；管不住的，助手在收紧之后再装一个 seccomp 过滤（`linux/seccomp.rs`）：`socket(AF_UNIX, …)` 回 `EACCES`，`socketpair` 照放；x86_64 上 32 位的照它的号挡 `socket`、整个挡 `socketcall`，x32 的、认不出的架构回 `EACCES`；aarch64 上 32 位的同样。探测多报 `landlock-sockets` 或 `seccomp`；核心照它设 `miyu_sandbox::set_confines_sockets`，两样都没有的成员、场所会话不跑命令，起来时记一行 `WARN sandbox cannot confine unix sockets`。测试用的 `MIYU_SANDBOX_SECCOMP=force` 在新内核上也走 seccomp（只会更严）。
3. 家目录：关起来的会话的命令 `HOME` 指到这一轮的工作目录（成员的工作区、场所的工作区），不再是跑核心的那个用户的家目录。CI 的 macOS 上量出来的：`git` 去读 `~/.gitconfig` 被 Seatbelt 拒（`Operation not permitted`），git 当成要命的错退出；Linux 上 Landlock 回 `Permission denied`，git 只警告，没暴露。顺带少透一样机主的身份（SEC-2）。
4. 已开着的成员会话：规格每次调用现拼，升级以后下一次调用就照新名单，工具面不变、不断缓存。

### 量出来的

本机（内核 7.2，有第 9 版）收窄后的名单里试：`whoami`、`id`、Python 的 `pwd`、`getpass`、`git init` 加提交、`curl https://example.com` 都照常；`cat /etc/shells`、`ls /home`、`ls /opt` 都被拒。

### 不做的、留给以后

- Windows（主会话定 A）：成员和场所会话照旧不跑命令，等 5-9 做出关得住读的沙盒（AppContainer 一类，`sandbox.md`「还没有的」）再放开。
- **SEC-2「成员会话透出机主的系统身份」**（主会话给的编号）：沙盒里 `pwd` 是真实路径（`/home/<机主用户名>/.local/share/miyu/home/<成员>/workspace`），`/etc/passwd`、`USER`、uid 都透出机主的用户名和数据根在哪（`HOME` 这一步已经换成工作目录），违背设计 06 U25 第 2 条（普通用户看不到运行它的那台电脑）。根子是成员和机主同一个系统用户：U19 每人一个系统身份才解；挂载命名空间把工作区挂到固定路径能缓解路径那一半。
- 老内核上走 seccomp 的成员、场所会话里 `miyu call` 用不了：它的通道是一个路径上的 Unix 套接字（`spec.sockets`），seccomp 看不到连的是哪个路径，只能整个挡。要用得改成交继承下来的描述符。
- `miyu doctor` 还没做（`cli/main.md` 列在「其余命令」里）：套接字管不住先靠运行日志那一行 `WARN`，`doctor` 做出来时一并报。

### 验收

1. `crates/miyu-session/tests/sandbox_member.rs`（真经助手跑 `shell`）：成员会话、群会话读得到自己的工作区、`/etc/hosts`、`git`；读不到别的成员的家目录、数据根、跑核心的用户的家目录、别处、`/etc` 名单外的（`shells`、`fstab`）、`/opt`；要出沙盒的拒、不问，拒的话里没有核心那边的绝对路径；群里主人对应表认出的账号开口、完全放开，照样关在群的工作区里；关不住读的平台上不跑。
2. `crates/miyu-sandbox/tests/linux_sockets.rs`（从 `linux.rs` 拆出来守 500 行，原来那两条套接字的测试随它搬）的 `a_confined_command_reaches_no_unix_socket_on_any_kernel`：路径上的、抽象的套接字在哪种内核上都连不上；强制走 seccomp 时能写的地方的也连不上（分得出走的哪一路）；`socketpair`、Python 照常；两样都没有的机器上不跑。
3. `crates/miyu-session/tests/sockets_off.rs`（自己一个进程）：套接字管不住时成员、群会话不跑命令，管理员本人的照跑；管得住以后照跑。`miyu-core` 的沙盒测试：管不住时记 `WARN`。
4. 手改：守卫不看套接字、判定恒真、seccomp 不装、挡错域、从不走 seccomp、名单多放一样，各至少红一条。
5. `cargo xtask check` 全过、Windows 版 clippy、三平台 CI（ubuntu-24.04 上成员会话真能跑命令，不再被「管不住」关掉）。
