## 施工单 WD-7：安装脚本的口语和三句拒绝原因

状态：做完了，等合并（分支 `step/WD-7-安装脚本和三句拒绝原因`，从 main 的 ebe63021d 开）。主会话 2026-11-22 派：WD-5 留下的两件。

### 一、安装脚本给人看的中文

`packaging/install.sh`（Linux、macOS）和 `packaging/install.ps1`（Windows）的中文输出是施工时写的口语（「没有装着」「连不上 GitHub」「核不过，不装」「不要了自己删」）。这一步按 `internal/blueprint/config.md`「用词的规矩」改写，和 `miyu upgrade` 说同一件事的句子用同一个说法（「GitHub 限流：匿名访问每小时 60 次，已经用完」「GitHub 返回 HTTP …」「未找到可用的发行版」「和清单不一致」）。

- 只改给人看的中文字符串。英文没有改动，脚本的逻辑、变量、退出码没有改动。
- `install.ps1` 里注释以外只能有 ASCII（`crates/miyu-source/tests/install_scripts.rs` 和脚本开头的说明），中文写成 `\uXXXX`。改写时先解码、换句子、再编码回去，换完以后注释以外仍然没有非 ASCII 字符。
- 签名用的「发版钥匙」改成「发版公钥」，和命令行的「这一版没有内置发版公钥」一致。
- 写进 `install.toml` 的那一行注释没有改动。
- `packaging/aur/miyu/miyu.install`（AUR 过渡包安装后的提示「运行 miyu migrate 把旧版数据搬过来」）没有改动：它单独发布在 AUR 上，仓库里改了而 AUR 没有更新时，手册引用的话会和人看到的对不上。

`install.sh`，30 句：

| 原来 | 现在 |
|---|---|
| `--version 后面要写版本` | `--version 后面需要写明版本` |
| `没有 ${os} ${arch} 的发行包（有 Linux x86_64、aarch64，macOS Apple 芯片）` | `没有 ${os} ${arch} 的发行包（目前有 Linux x86_64、aarch64 和 macOS Apple 芯片的发行包）` |
| `没有装着（${LIB} 不在）` | `未安装（${LIB} 不存在）` |
| `${LIB} 不是这个脚本装的（没有 install.toml），不动它` | `${LIB} 不是这个脚本安装的（没有 install.toml），没有修改它` |
| `开机启动的服务没拿掉，接着卸（之后自己 miyu service disable 不了的，照 packaging/README.md 手动删）` | `无法移除开机启动的服务，继续卸载（卸载以后不能再运行 miyu service disable，需要按 packaging/README.md 手动删除）` |
| `核心没停下，接着卸` | `无法停止核心，继续卸载` |
| `已卸载 Miyu。数据还在 ${MIYU_HOME:-${HOME}/.miyu}，不要了自己删。` | `已卸载 Miyu。数据仍然保留在 ${MIYU_HOME:-${HOME}/.miyu}，如果不再需要，可以自行删除。` |
| `连不上 GitHub（${url}）` | `无法连接 GitHub（${url}）` |
| `GitHub 限流了（匿名每小时 60 次），过一会儿再试` | `GitHub 限流：匿名访问每小时 60 次，已经用完，稍后重试` |
| `GitHub 回了 ${code}（${url}）` | `GitHub 返回 HTTP ${code}（${url}）` |
| `下载不成：$1` | `下载失败：$1` |
| `SHA256SUMS 是别的钥匙签的（${key_of_sig}），不装` | `SHA256SUMS 的签名来自其他密钥（${key_of_sig}），不安装` |
| `这台没有 openssl，没法核签名；只核 SHA-256。要核签名：gh attestation verify <文件> -R ${REPO}` | `这台机器没有 openssl，无法核对签名，只核对 SHA-256。如果要核对签名，运行：gh attestation verify <文件> -R ${REPO}` |
| `这台的 openssl 不认 Ed25519，签名没核（不是签名坏了）；只核 SHA-256。要核签名：gh attestation verify <文件> -R ${REPO}` | `这台机器的 openssl 不支持 Ed25519，没有核对签名（不是签名有问题），只核对 SHA-256。如果要核对签名，运行：gh attestation verify <文件> -R ${REPO}` |
| `SHA256SUMS 的签名核不过，不装` | `SHA256SUMS 的签名核对失败，不安装` |
| `签名核过了（发版钥匙 ${KEY_ID}）` | `签名核对通过（发版公钥 ${KEY_ID}）` |
| `要 curl` | `需要 curl` |
| `要 tar` | `需要 tar` |
| `已经装了 ${now}（${LIB}），升级用 miyu upgrade` | `已安装 ${now}（${LIB}）；如果要升级，运行 miyu upgrade` |
| `${LIB} 已经有了、里面却没有 miyu，先挪走它` | `${LIB} 已存在，但里面没有 miyu，需要先把它移走` |
| `${BIN}/miyu 已经有了（不是这个脚本放的），先挪走它` | `${BIN}/miyu 已存在（不是这个脚本创建的），需要先把它移走` |
| `没有找到可用的发行版` | `未找到可用的发行版` |
| `还没有正式版；要装预发布的加 --prerelease` | `还没有正式版；如果要安装预发布版，加上 --prerelease` |
| `${archive} 的 SHA-256 对不上，不装` | `${archive} 的 SHA-256 和清单不一致，不安装` |
| `解不开 ${archive}` | `无法解压 ${archive}` |
| `${archive} 里缺东西` | `${archive} 里缺少文件` |
| `新的 miyu --version 报的是「${reported}」，不是 ${version}` | `新版 miyu --version 报告的是「${reported}」，应为 ${version}` |
| `提示：${BIN} 不在 PATH 里，在 shell 的配置里加一行：` | `提示：${BIN} 不在 PATH 中，需要在 shell 的配置文件里加一行：` |
| `提示：PATH 里先找到的是 ${other}，不是刚装的这一份` | `提示：PATH 中先找到的是 ${other}，不是刚安装的这个版本` |
| `下一步：运行 miyu 开始；要开机就起，运行 miyu service enable。升级用 miyu upgrade，卸载用这个脚本加 --uninstall。` | `下一步：运行 miyu 开始使用；如果要开机启动，运行 miyu service enable。升级运行 miyu upgrade，卸载运行这个脚本并加上 --uninstall。` |

`install.ps1`，25 句（表里是解码以后的字）：

| 原来 | 现在 |
|---|---|
| `没有装着（{0} 不在）` | `未安装（{0} 不存在）` |
| `{0} 不是这个脚本装的（没有 install.toml），不动它` | `{0} 不是这个脚本安装的（没有 install.toml），没有修改它` |
| `开机启动的任务计划没拿掉，接着卸（之后照 packaging/README.md 手动删）` | `无法移除开机启动的任务计划，继续卸载（之后需要按 packaging/README.md 手动删除）` |
| `核心没停下，接着卸` | `无法停止核心，继续卸载` |
| `删不掉 {0}（{1}）：还有 Miyu 的程序开着的，关掉再卸` | `无法删除 {0}（{1}）：还有 Miyu 的程序正在运行，关闭以后再卸载` |
| `已卸载 Miyu。数据还在 {0}，不要了自己删。` | `已卸载 Miyu。数据仍然保留在 {0}，如果不再需要，可以自行删除。` |
| `GitHub 限流了（匿名每小时 60 次），过一会儿再试` | `GitHub 限流：匿名访问每小时 60 次，已经用完，稍后重试` |
| `连不上 GitHub（{0}）：{1}` | `无法连接 GitHub（{0}）：{1}` |
| `下载不成：{0}（{1}）` | `下载失败：{0}（{1}）` |
| `SHA256SUMS 是别的钥匙签的（{0}），不装` | `SHA256SUMS 的签名来自其他密钥（{0}），不安装` |
| `这台没有 openssl（Windows 自带的工具不认 Ed25519），签名没核（不是签名坏了）；只核 SHA-256。要核签名：gh attestation verify <文件> -R {0}` | `这台机器没有 openssl（Windows 自带的工具不支持 Ed25519），没有核对签名（不是签名有问题），只核对 SHA-256。如果要核对签名，运行：gh attestation verify <文件> -R {0}` |
| `这台的 openssl 不认 Ed25519，签名没核（不是签名坏了）；只核 SHA-256。要核签名：gh attestation verify <文件> -R {0}` | `这台机器的 openssl 不支持 Ed25519，没有核对签名（不是签名有问题），只核对 SHA-256。如果要核对签名，运行：gh attestation verify <文件> -R {0}` |
| `SHA256SUMS 的签名核不过，不装` | `SHA256SUMS 的签名核对失败，不安装` |
| `签名核过了（发版钥匙 {0}）` | `签名核对通过（发版公钥 {0}）` |
| `没有 {0} 的 Windows 发行包（只有 x86_64）` | `没有 {0} 的 Windows 发行包（目前只有 x86_64 的发行包）` |
| `已经装了 {0}（{1}），升级用 miyu upgrade` | `已安装 {0}（{1}）；如果要升级，运行 miyu upgrade` |
| `{0} 已经有了、里面却没有 miyu.exe，先挪走它` | `{0} 已存在，但里面没有 miyu.exe，需要先把它移走` |
| `没有找到可用的发行版` | `未找到可用的发行版` |
| `还没有正式版；要装预发布的加 -Prerelease` | `还没有正式版；如果要安装预发布版，加上 -Prerelease` |
| `{0} 的 SHA-256 对不上，不装` | `{0} 的 SHA-256 和清单不一致，不安装` |
| `解不开 {0}（{1}）` | `无法解压 {0}（{1}）` |
| `{0} 里缺东西` | `{0} 里缺少文件` |
| `新的 miyu --version 报的是「{0}」，不是 {1}` | `新版 miyu --version 报告的是「{0}」，应为 {1}` |
| `已把 {0} 加进你的 PATH：新开的终端里认得 miyu` | `已把 {0} 加入你的 PATH：新打开的终端里可以直接运行 miyu` |
| `下一步：运行 miyu 开始；要登录就起，运行 miyu service enable。升级用 miyu upgrade，卸载用这个脚本加 -Uninstall。` | `下一步：运行 miyu 开始使用；如果要登录时启动，运行 miyu service enable。升级运行 miyu upgrade，卸载运行这个脚本并加上 -Uninstall。` |

两份测试脚本 `test-install.sh`、`test-install.ps1` 只断言英文输出，不受影响。手册 `docs/zh/guide/install.md` 里引用的四句跟着修改。

### 二、三句拒绝原因

WD-5 里这三句没有找到一句能覆盖所有场合的说法。这一步逐个看了返回它们的每一处。

| 原因码 | 原来 | 返回的场合 | 这一步 |
|---|---|---|---|
| `local_only` | 仅限本机。／Local only. | 扩展调用只给人使用的方法；设备令牌连上的连接调用 `core.stop`、`account.setup_code`；扩展调用 `account.setup`、`login.ticket`；软件包越权修改配置 | 改成「只有本机上的用户可以执行这个操作。」／`Only a user on this machine can do this.`。原来的话说的是「在哪里」，而扩展本来就在本机；共同点是调用的不是坐在这台机器前的人，所以改成说「谁可以」 |
| `command_not_allowed` | 仅主人和群管理员可用。／Owner and group admins only. | 身份不够；命令被停用（`data.why` 是「/名字 已停用。」）；预设没有开这个技能或命令模板（`data.why` 说是哪个预设） | 身份不够时不变。后两种情形整句话改成「不能使用这条命令。」／`This command can't be used.`，原因仍然在 `data.why` 里。原来这两种情形 `message` 说「仅主人和群管理员可用」，`data.why` 说「已停用」，两句互相矛盾 |
| `unknown_server` | 设置里没有这个 MCP 服务。／No such MCP server in the settings. | `mcp.remove`、`mcp.restart` 只查设置；`mcp.login` 先查设置、再查已安装软件包带的服务 | 不改。这句话在每个场合都是真的：`mcp.remove` 一个软件包带的服务时，设置里确实没有它；`mcp.login` 时两处都没有，设置里同样没有。改成「MCP 服务不存在。」反而会在前一种场合说错 |

`command_not_allowed` 怎么区分情形：`command.run` 给每一次拒绝都带上 `data.why`，身份不够时 `data.why` 就是这一句本身（`crates/miyu-endpoint/src/commands/why.rs`）。所以 `crates/miyu-endpoint/src/refusal/message.rs` 的 `said` 按「`data.why` 和这一句不一样」来认另有原因的情形。原因码和 `data` 的字段都没有变化。

### 验证

- `sh -n packaging/install.sh` 通过；`install.ps1` 注释以外没有非 ASCII 字符。
- 在沙箱家目录里运行了 `install.sh` 三条不会安装任何东西的路径：

  ```
  $ HOME=<沙箱> sh packaging/install.sh --uninstall
  未安装（<沙箱>/.local/lib/miyu 不存在）

  $ HOME=<沙箱> sh packaging/install.sh --version
  miyu 安装：--version 后面需要写明版本

  $ HOME=<沙箱> MIYU_INSTALL_API=http://127.0.0.1:9 sh packaging/install.sh
  miyu 安装：无法连接 GitHub（http://127.0.0.1:9/releases/latest）
  ```

  完整的安装、卸载由 CI 的安装测试在干净的环境里运行。`install.ps1` 没有在 Windows 上实际运行，由 Windows 的 CI 覆盖。
- 拒绝原因：`crates/miyu-endpoint/src/refusal/message/tests.rs` 加一条测试（三种情形各说什么）；`crates/miyu-endpoint/tests/command_access.rs` 在命令停用的那一处多断言整句话；`miyu-endpoint` 的全部测试通过。
- 门禁十二项、三个平台的 CI 通过。

### 留给其他会话的

- 各界面如果钉了 `local_only` 的「仅限本机。」，或者命令停用时的整句话，换钉时按上表修改。
- `packaging/aur/miyu/miyu.install` 的那一句要改的话，需要和 AUR 上的发布一起做。
