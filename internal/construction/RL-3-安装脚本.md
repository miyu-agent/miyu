## RL-3 安装脚本：`install.sh`、`install.ps1`

状态：施工完，待 core1 审（2026-11-05，sub1）。设计 `59-发版.md` 第三节第 2 条、第四节、第六节第 3 条；`12-进程形态与分发.md`
第五节、第六节；RL-4（`miyu upgrade`）、蓝图 `cli/upgrade.md`。

### 目的

给不用包管理器的人一行命令装上 Miyu。装好的那一份 `miyu upgrade` 认得出是脚本装的，能自己升级。卸的时候也是同一个脚本。

### 定了的（core1 11-05）

1. **签名**：
   - 先核签名再核 SHA-256。签名照写在脚本里的发版公钥，用 `openssl pkeyutl -verify -rawin` 核（要 OpenSSL 3）。
   - 这台的 openssl 不认 Ed25519 的（macOS 自带的 LibreSSL；Windows 自带的工具也不认），说一句「这台的 openssl 不认 Ed25519，
     签名没核（不是签名坏了）」，提示用 `gh attestation verify`，接着只核 SHA-256。
   - 核得了却核不过的，一律停下，不装。
2. **装在哪**：要和 `miyu upgrade` 认的一致。
   - Linux、macOS：压缩包第一层整个放进 `~/.local/lib/miyu/`，`~/.local/bin/miyu` 链过去。PATH 里没有 `~/.local/bin` 的只说怎么加，
     不改 shell 的配置。Linux 另装 `.miyu` 文件类型。
   - Windows：`%LOCALAPPDATA%\Programs\Miyu\`，加进当前用户的 PATH。
   - 都在程序旁边写 `install.toml`（`channel = "script"`）。
3. **参数**：
   - 默认装最新正式版；
   - `--version`（`-Version`）装指定的一版；
   - `--prerelease`（`-Prerelease`）也算预发布，同 `miyu upgrade`；
   - 已经装了的说一句「升级用 miyu upgrade」就退出。
4. **卸**：`--uninstall`（`-Uninstall`）。
   - 先 `miyu service disable`、`miyu core stop`，停不了的说一句、接着删；
   - 删掉安装目录、链接或 PATH 项、文件类型；
   - 数据根不碰，说一句它在哪。
5. **实测以 CI 为主**：
   - 新 workflow 在 ubuntu、macos、windows 上装、核、卸，资产用公开的 rc.3；
   - 容器里另跑 Arch、Fedora；
   - 主人的 Mac Studio、gaming 虚拟机不动。

### 做什么

- **`packaging/install.sh`**（POSIX sh，macOS 的 bash 3.2 也能跑，shellcheck 干净）：
  - 认系统：`linux-x86_64`、`linux-aarch64`、`macos-aarch64`，别的报没有发行包。
  - 先查已经装了没有。`~/.local/lib/miyu` 在却没有 `miyu`、`~/.local/bin/miyu` 是别的东西的，都停下，让人先挪走，不覆盖。
  - 找标签：默认 `/releases/latest`，404 说「还没有正式版，加 --prerelease」；`--prerelease` 取 `/releases?per_page=30` 的第一个；
    限流、别的状态码分开说。
  - 暂存放在 `~/.local/lib/.miyu-install.XXXXXX`，和装的地方同一块盘，退出时删掉。
  - 下载 `SHA256SUMS`、`SHA256SUMS.sig`，核签名：
    - 签名文件的 `key` 要是写在脚本里的编号；
    - 用 RFC 8410 的 DER 前缀拼出 PEM；
    - 先探这台 openssl 认不认：读得了这把公钥、`pkeyutl` 有 `-rawin`。
  - 照清单那一行核 SHA-256，解开后要有 `miyu`、`resources/`，跑新的 `miyu --version` 要报这一版。
  - 写 `install.toml`，暂存改名成安装目录，链接，Linux 装文件类型。
  - 收尾的提示：PATH 里没有 `~/.local/bin`，PATH 上先找到的是别处的 `miyu`（例如旧的 AUR 包），下一步怎么用。
  - 话照 `LANG` 挑中文或英文。测试用 `MIYU_INSTALL_API`、`MIYU_INSTALL_BASE` 换地址。设了 `GITHUB_TOKEN` 的，问 API 时带上。
- **`packaging/install.ps1`**：同一套步骤，Windows PowerShell 5.1、PowerShell 7 都能跑。
  - 错误用 `throw`：照说明里 `& ([scriptblock]::Create((irm …)))` 跑的不关窗口，`-File` 跑的退出码是 1。
  - PATH 上有 openssl 的（例如 Git for Windows 带的）照样核签名。
  - `install.toml` 写成不带 BOM 的 UTF-8。
  - 话照 `LANG` 挑（同 `install.sh`），没设的照界面语言。实测另设 `LANG=zh_CN` 跑一次，输出写进 UTF-8 文件，查中文提示解得出来。
- **实测**：
  - `packaging/test-install.sh`、`test-install.ps1` 在干净的家目录里走一遍：
    - 没有正式版时不加 `--prerelease` 停下；
    - `--prerelease` 装上，链接、`install.toml`、PATH 在，`miyu --version` 对得上，没留暂存；
    - **装完跑 `miyu upgrade --prerelease` 要说「已是最新」**：脚本装的和 upgrade 认的是同一套；
    - 再装说已经装了；
    - 有 python 的另起本机下载处：压缩包改过的停在 SHA-256；`SHA256SUMS` 改过的，openssl 认的停在签名，不认的要说核不了；
    - 卸掉以后都没了、数据根还在，再卸说没装；
    - Windows 主流程用 5.1 跑，最后用 7 再装、卸一遍。
  - `.github/workflows/install-test.yml`：改了这几个文件才跑，也能手动跑。
- **守着抄的两样**：`crates/miyu-source/tests/install_scripts.rs` 查两个脚本里的发版公钥是 `sign::RELEASE` 的最后一把，`install.sh` 写的
  文件类型和 `packaging/mime/miyu.xml` 一样。
- **文档**：`packaging/README.md` 加安装脚本一段，设计 59 第六节指过来。仓库首页的 README 还写着「还不能用」，安装命令等发版那天
  再加。

### 怎么验

- 容器（非特权 docker）里跑 `test-install.sh`，都过：
  - Ubuntu 24.04：openssl 3，核了签名，两种改过的下载都停下；
  - Arch：核了签名，没有 python，跳过改过的那两条；
  - Fedora：镜像里没有 openssl，说核不了，只核 SHA-256。
- CI 三台：见交付时的 run。
- `install_scripts` 两条测试过。

### 变异

- 手写 6 个改坏 `install.sh` 的，各在 Ubuntu 容器里跑实测脚本。
  - 抓住 5 个：签名核不过照装、SHA-256 对不上照装、不写 `install.toml`、卸的时候不删链接、没有正式版也算成。
  - 「不删链接」第一次没抓住：实测用 `-e` 查，悬空的链接 `-e` 是假，改成也查 `-L` 以后抓住。
  - **「不查新版报的版本」抓不住**：要造一份版本不对、签名却核得过的发布，没有发版私钥造不出来。这一条和 `miyu upgrade` 第 8 步同一个意思，
    那边有单元测试。
- 公钥、文件类型两条：改了脚本里的公钥、glob，`install_scripts` 红。

### 坑

- **子 shell 里的 `die` 退不出脚本**：`t=$(target)` 里 `exit` 只退子 shell，赋值后面要接 `|| exit 1`。
- **GitHub 的回应可能压成一行**：照逗号拆开再找 `tag_name`，不然贪婪匹配拿到的是最后一个。
- **PowerShell 5.1 里外部程序往标准错误写会变成错误**：`$ErrorActionPreference = 'Stop'` 时会抛出来。跑 openssl、miyu 的那几处
  局部改成 `Continue`。
- **Windows PowerShell 5.1 和 BOM 两头堵**：
  - 不带 BOM 的 `.ps1` 用 `-File` 跑时照系统代码页（ANSI）解，字符串里的中文字节会冒出它认作引号的弯引号，字符串就断了，整个
    脚本报语法错（CI 第一趟）；
  - 带了 BOM，`irm` 交回的字符串开头留着 BOM 字符，`[scriptblock]::Create` 认不出开头的注释块，照说明的用法跑不起来（CI 第三趟，
    实测里照这条路跑的那一条红了）。
  - 定下来的做法：`install.ps1` 不带 BOM，注释以外只有 ASCII。给人看的中文写成 `\uXXXX`，照 `[regex]::Unescape` 解出来，再用 `{0}`
    填值，路径里的反斜杠不经过 Unescape；注释里的中文读乱了不影响解析。`install_scripts` 加一条测试守着。`test-install.ps1` 只由
    PowerShell 7 跑，照旧带 BOM。
- **PowerShell 的变量名不分大小写**：全局的 `$Zh` 和函数参数 `$zh` 是同一个变量，`Die` 里永远选中文。CI 第二趟红在这里，
  全局的改名 `$UseChinese`。
- **匿名的 GitHub API 每小时 60 次，GitHub 的托管机器共用出口**：CI 第二趟 macOS 那台第一次问就被限流了。
  - 两个脚本设了 `GITHUB_TOKEN` 的，问 API 时带上它（只发给 API，下载不带），workflow 传 `github.token`。
  - `miyu upgrade` 照旧匿名：实测里它撞了限流的，印一句 WARNING 说这一趟没验成「已是最新」，别的照查。容器里、ubuntu 那台验过。
