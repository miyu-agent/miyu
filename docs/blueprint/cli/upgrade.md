## `miyu upgrade`

### 是什么

升级 Miyu 本身（施工 RL-4，设计 `12-进程形态与分发.md` 第五节 R10「谁装的谁升级」、`59-发版.md` 第三节第 2 条、第四节）：安装脚本
装的，自己下载、核对、替换；AUR、Homebrew 装的，说用哪条包管理器的命令；手工解开压缩包的，只说去哪下。不连核心，数据根一个字节
都不碰。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu/src/main.rs` | 子命令 `upgrade`；换上帮助页 |
| `crates/miyu-cli/src/upgrade.rs` | 参数（`--prerelease`、`--yes`）、入口、要说的那一句（`Said`）和退出码；真跑时的程序位置、网络、问一句、跑新版的 `--version` |
| `crates/miyu-cli/src/upgrade/plan.rs` | 只算不做：安装标记认谁装的、版本号怎么比、发布列表里挑哪一个、压缩包叫什么、校验和清单里它那一行 |
| `crates/miyu-cli/src/upgrade/flow.rs` | 怎么走（下面「怎么走」），经 `World` 拿网络、程序位置、公钥，测试换替身 |
| `crates/miyu-cli/src/upgrade/net.rs` | 下载：`miyu-http` 的一次 GET，照环境变量走代理；连不上、限流、别的状态码分开 |
| `crates/miyu-cli/src/upgrade/swap.rs` | 解开 tar.gz、zip，查新版缺什么，和装着的逐样对换、出错照原样退回，收拾上次留下的 |
| `crates/miyu-cli/src/language/upgrade.rs` | 给人看的字，中英文 |
| `crates/miyu-cli/src/help/{zh,en}/upgrade.txt` | 帮助页 |
| `crates/miyu-source/src/sign.rs` 的 `RELEASE`、`release()` | 编进来的发版公钥（和软件源的 `OFFICIAL` 分开）；`miyu_source::sums::check` 查校验和清单的写法（施工 RL-2） |

### 安装标记

程序真实位置（解开链接）的旁边放 `install.toml`，写谁装的：

```toml
channel = "script"   # script：安装脚本；aur：AUR 的包；brew：Homebrew 的配方
```

安装脚本（施工 RL-3）写 `script`；AUR、Homebrew 的 `miyu-core` 包（施工 RL-5，设计 60）写 `aur`、`brew`。没有这个文件的（手工
解开的压缩包）当没标记。`#` 起的是注释，别的键不看；`channel` 写的不是这三个、不是带引号的字的，报读不懂。

### 怎么走

1. **认谁装的**：读标记。没有的说「不自动升级」、提示下载地址 `https://github.com/miyu-agent/miyu/releases/latest`，退出码 0；
   `aur` 说用 `paru -Syu`、`brew` 说用 `brew upgrade`（升整套，不猜装了哪几个包），退出码 0；读不懂的报错。只有 `script` 往下走。
2. **收拾上次的**：删掉安装目录里上次留下的 `.miyu-old-<版本>/`、`.miyu-upgrade/`（Windows 上跑着的程序删不掉，留到这一次）。
3. **公钥**：编进来的发版公钥表（`sign::RELEASE`，施工 RL-4补 填上主人的第一把 `ca6918041857843c`）是空的时候，报「这一版没有内置
   发版公钥」，不升级、不跳过签名。系统不在发的四种里的
   报没有这种系统的发行包。这两样都在上网之前。
4. **问 GitHub**：`/repos/miyu-agent/miyu/releases?per_page=30`，草稿不算，预发布只在 `--prerelease` 时算，标签要是 `v<版本>`。不比现在这一版
   （`miyu --version` 的那一个）新的说「已是最新版本」，退出码 0：只升不降。一个能用的都没有时：不带 `--prerelease`、列表里有预发布的说
   「还没有正式版」、提示加 `--prerelease`（施工 RL-4补2），连预发布都没有的说「没有找到可用的发行版」，都是退出码 1。连不上、限流（`403`、`429` 带 `x-ratelimit-remaining: 0`，
   匿名每小时 60 次，照 `x-ratelimit-reset` 说几点以后再试）、别的状态码分开说，退出码 1。
5. **问一句**「升级 Miyu <旧> → <新>？[Y/n]」，`--yes` 不问；答不升的「已取消」，标准输入关着的报没有收到确认、提示加 `--yes`，
   都是退出码 1。
6. **核对**：发布里要有 `miyu-<版本>-<系统>.tar.gz`（Windows `.zip`，同 `cargo xtask dist`）、`SHA256SUMS`、`SHA256SUMS.sig`。
   签名照发版公钥验（签法同软件源的索引，`sign::verify`），清单照 `sums::check` 查写法，里面要有这个压缩包那一行。哪一样不对都停。
7. **暂存**：在安装目录里建 `.miyu-upgrade/`（和装着的同一块盘，对换只是改名）。没有写权限的说清楚、提示不提权，退出码 1：
   不 `sudo`、不提权；装进系统目录又没写标记的也停在这里。
8. **下载、查新版**：下载压缩包，SHA-256 要和清单一致；解开（不许写到目录外面），`miyu-<版本>-<系统>/` 里要有五个程序和
   `resources/`；跑新的 `miyu --version` 要报 `miyu <新版本>`。
9. **对换**：照安装目录的第一层分（施工 RL-4补3，core1 11-05 定）。Miyu 自己的整样换：新压缩包第一层有的每一样，加上一张固定的名单
   （`miyu`、`miyu-tui`、`miyu-web`、`miyu-sandbox`、`miyu-extract`，Windows 带 `.exe`；`resources`、`LICENSE`、`README.txt`、
   `THIRD-PARTY-LICENSES.txt`：以后新版拿掉某个程序，旧的那份也清得掉）；`install.toml`、暂存、`.miyu-old-…` 不动；第一层别的东西
   原样留着，最后说一句「安装目录里还有 N 样不是 Miyu 的，原样留着」、列最多 5 样。`resources/` 整个换，放在里面的照样没了（帮助页
   写着安装目录别放自己的东西）。要换的每一样挪进 `.miyu-old-<旧版本>/`，新版的每一样挪进来，删掉 `.miyu-old-…`。逐样挪、不整个
   目录改名：Windows 上跑着的 `miyu.exe` 改得了名、删不掉，目录却改不了名。中途哪一步出错，照反方向挪回去，留着旧的，报哪一步、
   系统的原话，退出码 1。删不掉的旧版目录说一句，下一次 `miyu upgrade` 开头删。
10. **说**：「已升级 Miyu <旧> → <新>」，提示已经在跑的核心还是旧版、`miyu core stop` 以后下次打开就是新版。头请求核心空闲时重启
    （设计 04 第八节 `kernel.restart_when_idle`）还没有，有了以后照它走。

停下的地方（第 1 到 8 步）都删掉暂存，装着的那一份不动。

### 对外的样子

```text
$ miyu upgrade
升级 Miyu 2026.11.0 → 2026.11.1？[Y/n]
正在下载 miyu-2026.11.1-linux-x86_64.tar.gz
正在核对新版
正在替换
已升级 Miyu 2026.11.0 → 2026.11.1
提示：已经在跑的核心还是旧版：运行 'miyu core stop'，下次打开就是新版
```

```text
$ miyu upgrade
这份 Miyu 由 AUR 安装，用包管理器升级
  paru -Syu
```

成了（升好了、已是最新、包管理器的、没标记的）印在标准输出上，退出码 0；别的照报错写（`miyu: <事>`、`提示：<怎么办>`），印在
标准错误上，退出码 1。

### 测试

| 测试 | 守着什么 |
|---|---|
| `upgrade/plan/tests.rs` | 标记的几种写法和写错；版本号读法、照 semver 比（预发布排在正式版前、`rc.10` 在 `rc.9` 后）；草稿、预发布不算，`--prerelease` 才算；压缩包名同 `dist`；清单照文件名找那一行 |
| `upgrade/swap/tests.rs` | tar.gz、zip 都解成一层目录，解不开的报错；新版缺的程序、`resources/`；对换后新的在、标记留着、旧的删了；挪到第几步出错都退回原样；收拾只删上次留下的 |
| `upgrade/tests.rs`、`upgrade/tests/refused.rs`、`upgrade/tests/words.rs` | 假网络、真钥匙、真压缩包走一趟：升好；包管理器、没标记的不上网；标记写错、没有公钥、没有这种系统在上网之前停；连不上、限流、状态码、不是 JSON；已是最新、不降级、预发布要 `--prerelease`；答不升、没得答、`--yes` 不问；没有签名文件、别的钥匙签的、清单被改过、压缩包被改过、清单里没有这一行、清单写坏；新版缺东西、报的版本不对；没有写权限；对换中途出错。每一种停下的都查装着的没动、暂存删了。两种语言每一句都说得出 |
