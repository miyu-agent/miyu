## 施工单 PV-RM：去掉私密目录

状态：做完，等合（分支 `step/PV-RM`）。项目主人 2026-11-16 定。

### 目的

只读模式就是「完全放开减去写」，Miyu 不替人藏东西；想藏的人自己布沙盒。所以施工 5-13 到 5-17 做的「别的程序的私密目录」连同设置一起去掉：出厂清单（三个平台几十项，`~/.ssh`、浏览器、密码管理器……）、照环境变量（`GNUPGHOME`、`KUBECONFIG`……）算出的位置、配置项 `sandbox.private.enabled`、`.add`、`.remove`，只读时命令读不到它们、文件工具读之前先问的那一套。

**Miyu 自己的数据根照旧哪一级都碰不到**，这一条一个字节都不松：边界表的 `Forbidden`、沙盒规格的 `hidden` 里的数据根、往下走目录的篱笆里的数据根、工具执行时核对判过的位置，都不动。

没发版，不留兼容：配置里写着 `sandbox.private.*` 的，照不在清单里的键报 `unknown_key` 警告（同 AUD-4 的 `permission.level`、LV-5 的 `sandbox.*`）。

### 摸底清单（动手前列的）

**一、只给私密目录用的：删**

| 在哪 | 删什么 |
|---|---|
| `miyu-fs/src/private.rs`、`private/shipped.rs`、`private/moved.rs`、`private/tests.rs` | 出厂清单 `SHIPPED_PRIVATE`（Linux、macOS、Windows、别的 Unix 四份）、照环境变量挪去的位置（`moved.rs`）、`private_dirs`、`private_dirs_with`、`too_broad` |
| `miyu-fs/src/boundary.rs` | `Zone::Private`、`Places::private`、`Boundary` 里的私密目录、`zone` 里判私密的那一段 |
| `miyu-tool/src/private.rs`、`private/tests.rs` | 三项配置 `ENABLED`、`ADD`、`REMOVE` 和 `dirs`、`dirs_in` |
| `miyu-session/src/guard.rs`、`guard/bounds.rs`、`guard/levels.rs` | 权限策略的 `private` 一格、`private_of`、`set_private`；判法表的 `Mark::Ask`（只读时读私密目录问人，A16 起报路径的调用问人只剩这一种） |
| `miyu-session/src/guard/asking.rs` 的 `Asked`、`ask`、`granted`、`scope` | 报路径的调用问人、提放行规则、放行过的规则管不管得到（施工 D-1）：生产代码里提规则的只有它，没了 `Mark::Ask` 就是死代码。留下的 `hazardous` 挪进 `guard/hazardous.rs` |
| `miyu-session/src/sandbox.rs`、`tools/back.rs`、`actor/model.rs` | 沙盒的 `private`、`set_private`、回合开始照配置换私密目录、`outermost`（藏的几个彼此不重叠，只为私密目录） |
| `miyu-basesystem/src/walk.rs` | 篱笆 `Fence` 的 `private` |
| `miyu-endpoint/src/config/private.rs` 的 `sandbox.private.add`、`.remove` 那两道查、`options`、`ticks`、`tick` | `config.set` 时私密目录多查的一道（`private_too_broad`、`not_an_option`），`config.schema` 里的出厂清单选项和 `ticks: "unlisted"`；`config/set.rs` 为它传的家目录 |
| `miyu-config` 的 `Code::PrivateTooBroad`、`Code::PrivateNotShipped` 和两句话 | 出错码 `private_too_broad`，`config/private-not-shipped`、`config/private-too-broad` |
| `miyu-core/src/settings.rs` | 登记 `miyu_tool::private::ITEMS` 的那一行 |
| `resources/core/human/{zh,en,ja}.json` | 三项的名字、说明，设置页组 `private` 的名字，上面两句话 |

**二、私密目录和数据根共用的：只删私密目录那一份，数据根那一份不动**

| 在哪 | 怎么分 |
|---|---|
| 沙盒规格的 `hidden`（`miyu-session/src/sandbox.rs` 的 `hidden`） | 原来是「数据根，加上这台机器上有的私密目录」。数据根那一路（落在能写的里面只在 macOS、Windows 上藏，Landlock 照 dsh 不挖洞）原样留下，私密目录那一路删掉；函数挪成不带 `self` 的 |
| 边界表 `zone` | 数据根那一段（`Forbidden`）、技能文件夹（`Shelf`）原样；私密那一段删掉。工作区、自己的几处、加进来的目录照旧算能写 |
| 篱笆 `Fence::from`、`keeps_out` | 数据根照旧挡、照旧剪掉碰不到的；私密目录那一份删掉 |
| `Spec::hides`、`miyu_call::Channel::sandboxed` 的放开核心程序、Windows 的 `deniable`、`carve` | 通用的：藏起来的那一处包住了装的位置就单独放开、独占打开的不交、挖路。数据根照旧用得到，代码不动，只改注释 |
| `miyu-fs/src/private.rs` 的 `written` | 别的技能目录 `skills.external.dirs` 也照它拼。挪进 `miyu-fs/src/written.rs`，不删 |
| `miyu-endpoint/src/config/private.rs` 的 `external_dirs`、写法不对的那一道 | 别的技能目录用的。文件改名 `config/dirs.rs`，出错码 `Code::PrivatePath` 改名 `Code::DirPath`（协议上照旧是 `bad_format`），那一句改名 `config/bad-dir-path`，字一样 |

**三、确认框「本会话都允许」（「此目录不再询问」）**

- 生产代码里提放行规则（`rule`）的只有权限策略「只读时读私密目录」那一条；不报路径的命令、高危命令、装包卸包、扩展的守卫一直都不提。所以权限策略里管规则的那一份随私密目录删了（上表），`Guard::judge` 不再收 `grants`。
- 内核那一套（`tool.approval_requested` 的 `rule`、`Decision::Session`、`Decision::Workspace`、本会话放行过的 `grants`、`no_rule`）是执行前那条链的通用写法，也给以后的链用，这一步留着：没人提规则，内核照旧拒「没提规则却选了记住的」（`no_rule`），头照有没有 `rule` 决定给不给那一项，所以界面上那一项自然不出现。要不要连内核这一套也拆，另报拍板（施工单最后一节）。

**四、日志、协议**

- 会话日志：没有私密目录专有的事件。`tool.approval_requested` 的 `detail` 里施工 5-13 起有 `paths`（每一条 `path`、`write`、`zone`，`zone` 写 `private`、`outside`）、`rule` 有 `read`：报路径的调用不问人了，权限策略发的请求不再有这几格。事件的样本 `internal/designs/samples/events/tool.approval_requested.jsonl` 换成只读时沙盒用不了、`shell` 问人的那一条（没有 `rule`）。
- 运行日志：没有私密目录专有的行，`cargo xtask logs` 照旧。
- 给模型看的字：没有提私密目录的（`cargo xtask prompts` 照旧）。

**五、测试**

| 文件 | 怎么办 |
|---|---|
| `miyu-session/tests/guard_private.rs`、`guard_grants.rs` | 删：守的是私密目录问人、放行规则 |
| `miyu-session/tests/sandbox_private.rs` | 换成 `secrets_unreachable.rs`：只读的规格里藏的只有数据根；出厂的 `read` 完全放开、只读都读不到数据根里的 `etc/private/secrets.toml`；只读时读 `~/.ssh/config` 不问人 |
| `miyu-session/tests/sandbox.rs` 的 `a_read_only_command_cannot_read_inside_a_private_dir` | 换成 `a_read_only_command_reads_a_key_folder_but_not_the_secrets`：真经助手跑，钥匙读得到、写不进，密钥读不到 |
| `miyu-session/tests/guard_checked.rs` 的 `a_link_swapped_while_asking_is_not_followed_into_the_data_root` | 保数据根的，不删、不改松：原来靠只读读私密目录问人停住，换成扩展挂的守卫问人停住（施工 HK-1，生产上照样有这个空当），断言一字不变 |
| `miyu-session/tests/guard_thread.rs` 的 `asking_writes_what_the_sample_shows` | 照新的样本比：沙盒用不了时 `shell` 问人，不提规则 |
| `miyu-session/tests/support/site.rs` 的 `on_site` | 删：只为滤掉照环境变量另藏的私密目录；用到它的三处直接比整份规格，断言更严 |
| `miyu-session/src/sandbox/tests.rs`、`guard/tests.rs` | 留数据根的几格，去掉私密目录那几格 |
| `miyu-fs/tests/boundary.rs` | 私密目录的两条换成「`~/.ssh` 在边界以外，数据根照旧不能碰」「工作区包住数据根，数据根照旧不能碰」 |
| `miyu-basesystem/tests/walk_private.rs`、`src/walk/tests*.rs` | 换成 `walk_home.rs`：从家目录搜下来 `~/.ssh` 里的照搜；篱笆的对照测试只剩数据根，照旧逐个目录对照 |
| `miyu-endpoint/tests/config_set_private.rs` | 私密目录那两条删，别的技能目录那一条挪进 `config_set_dirs.rs` |
| `miyu-endpoint/tests/answer.rs` | 问人换成只读时沙盒用不了的命令；「本会话都允许」那一条换成 `no_rule` |
| `miyu-endpoint/tests/config_protected.rs`、`miyu-core/tests/settings*.rs`、`miyu/tests/config.rs` | 清单、组、`config get` 的输出去掉那三项 |
| 新加 `miyu-core/tests/settings_removed.rs` 的 `the_private_dirs_keys_only_warn_as_unknown` | 改前红：配置里写 `[sandbox.private]` 三项，只报三条 `unknown_key` 警告、不进最终值，别的项照读 |

### 顺手一处：只读下联网不问（主会话 2026-11-16 转来）

语音会话实测看到只读下 `deepseek_status`（访问类别 `network`）要人批。照「只读就是完全放开减去写」，联网不算写：

- 原来：不报路径的调用里，`network` 只读问人、完全放开放行（`guard/untargeted.rs`）。现在：两级都放行。只这一处判法。
- 命令里联网本来就不管：沙盒只管读写，不管网络（`sandbox.md`，2026-09-29 项目主人定），只读、完全放开一样。
- `web_search`、`web_fetch` 登记的访问类别是 `read`，本来就不问；受影响的是扩展声明 `network` 的工具、MCP 的工具。
- 没动：`outbound`（对外发消息）、不认识的类别照旧只读问人（是不是算「写」没定，另报）。
- 测试：`guard/tests.rs` 的 `a_call_without_paths_goes_by_what_it_does` 改成只读联网放行、对外发消息照问（改前红：判下来是 `Ask`）；新加 `tests/guard.rs` 的 `a_network_tool_runs_in_read_only_without_asking`（只读、没人能确认也照跑）；`nested_asking.rs` 原来借只读联网问人造「脚本里调的要人确认」，换成沙盒用不了时的执行命令，断言不变。

### 协议上去掉了什么（给头）

- 配置项 `sandbox.private.enabled`、`sandbox.private.add`、`sandbox.private.remove`：`config.schema` 里没有了，「隐私与安全」那一页的组 `private`（「私密目录」）没有了；`config.get`、`config.set` 带这几个键的回 `unknown_config_key`。设置页「隐私与安全 → 私密目录」那一块拿掉。
- `config.schema` 的 `ticks`（施工 5-15 再补，只有 `sandbox.private.remove` 带 `"unlisted"`）：整格去掉。
- 出错码 `private_too_broad`：去掉。`not_an_option` 照旧有（别处用）；别的技能目录写错的照旧是 `bad_format`。
- `tool.approval_requested`：`by` 是 `permissions` 的请求不再带 `rule`，`detail` 不再有 `paths`（连同 `zone: "private"`、`"outside"`）；不报路径的那种（`tool`、`command`、`title`、`sandbox`、`hazard`）照旧。头上「本会话都允许」那一项照有没有 `rule` 决定，自然不出现。
- 只读时文件工具读 `~/.ssh` 这些不再问人，命令读得到（写照旧不行）。

### 蓝图改哪几节

- `config.md`：清单去掉两行、说明去掉那一条，「去掉的」加 PV-RM 一条；出错表去掉 `private_too_broad`，`bad_format` 那一行改说法；`ticks` 那一条去掉；字的表去掉三项；守着它的两行；参考文件的样本块照重新生成的换。
- `session/guard.md`：边界表、判法表去掉私密目录，「放行过的不问」、「问人时提的规则」改成不提，文件表、测试表。
- `session/tools.md` 第 1a 条、测试表；`sandbox.md` 的 `hidden`、测试表；`sandbox/linux.md`、`sandbox/windows-run.md` 顺手的几句。
- `fs.md`：`Zone`、`Places`、`written`，边界表的先后，测试表。
- `tools/glob.md`：篱笆不再挡私密目录，测试表。
- `kernel/asking.md`「本会话放行过的」：核心自带的链都不提规则了。
- `protocol.md` 的 `config.schema`、太宽那一句；`providers.md`、`tools/shell.md`、`tools/interface.md` 各一句。
- 协议参考 `config.md`（中英）：去掉 `ticks`、`private_too_broad`；`sessions.md`（中英）：选 `session` 记住的是什么。
- `docs/guide`、`docs/dev` 由写文档的会话另改，这一步不碰；设计 `11-权限与沙盒.md` 的批注也留给它。

### 验收

1. 改前红：`miyu-core` `settings_removed.rs` 的 `the_private_dirs_keys_only_warn_as_unknown`，在 `origin/main`（4addbfd91）上跑是红的（那三项还在清单里，进了最终值、不报），这一版绿。
2. 保护数据根的测试一条没删、没改松：`miyu-session` 的 `guard.rs` `the_data_root_is_refused_at_every_level`、`guard_checked.rs` 的链接调包、`sandbox.rs` 的 `commands_really_run_in_the_sandbox`、`sandbox_member.rs`、`sandbox_skills.rs`，`miyu-basesystem` 的 `audit_data_root.rs`、`glob.rs`、`grep.rs`，`miyu-endpoint` 的 `files.rs`、`reads.rs`、`attach.rs`、`provide_paths.rs`，`miyu-fs` 的数据根几条照旧绿。新加的 `secrets_unreachable.rs` 三条、`sandbox.rs` 的 `a_read_only_command_reads_a_key_folder_but_not_the_secrets` 绿。
3. `miyu-core` `settings.rs`：三种语言的字和清单对得上，生成的参考文件、Schema 和样本逐字节一样（样本重新生成）。
4. 门禁十一项、`cargo test -p miyu-kernel random:: -- --include-ignored`、`cargo clippy --workspace --all-targets --target x86_64-pc-windows-gnu -- -D warnings`、`cargo xtask prompts`、`cargo xtask logs`。

### 实测（真模型，沙箱数据根）

- 环境：`cargo xtask dev-home` 造的数据根（`MIYU_HOME` 指草稿目录），`HOME` 也指沙箱里的假家，模型照 magpie 网关的 `deepseek-v4-1-flash`；配置写 `[permission] start_read_only = true` 和 `[sandbox.private] add = ["~/notes"]`；假家里造 `~/.ssh/id_test`（`FAKE-SSH-KEY-PVRM-OK`），数据根里造 `etc/private/secrets.toml`（假的标记）。用这一版 `cargo build` 出来的 `miyu`、`miyu-sandbox`。
- `miyu check`：`etc/config.toml:17:1 警告：没有 sandbox.private.add 这一项。已忽略此行。`（退出码 0，另两条是没设环境变量的警告）。
- `miyu ask --permission read-only`（没人能确认，要问人的会直接拒）：她调 `read` 读 `~/.ssh/id_test` 成功，结果第一行 `FAKE-SSH-KEY-PVRM-OK`；读 `secrets.toml` 被拒，原文 `"…/data/etc/private/secrets.toml" is inside Miyu's own data, which no tool can read or change.`。会话日志里没有一条 `tool.approval_requested`，会话的权限是 `{"level":"full","read_only":true}`。
- 头一趟她自己改用 `shell`（只读沙盒里跑）：`~/.ssh/id_test` 读得到内容（`file` 判出 ASCII text），`secrets.toml` 读不到（`grep: …secrets.toml: 权限不够`）。
- 联网那一处没在沙箱里实测：沙箱里没有声明 `network` 的工具（`web_search`、`web_fetch` 是 `read`），只有上面的测试。
- 测完按 `MIYU_HOME` 找到自己起的核心停掉；key 只放在命令的环境变量里，草稿目录、工作树里数过没有。

### 不做什么

- 完全放开时命令不进沙盒（施工 LV-1），数据根只由文件工具、往下走目录的工具、高危命令里删数据根的那一条挡；命令在完全放开下读得到数据根。这一步不改，另报拍板。
- 内核的放行规则那一套（见上第三节）不拆。
- 数据根不在 `~/.miyu` 时，旧版 Miyu 的 `~/.miyu`（原来在出厂清单里）不再藏：只有这个核心自己的数据根受保护。

### 拿不准、要拍板的

1. 完全放开下命令读得到数据根里的密钥（见「不做什么」第 1 条）：要「哪一级都碰不到」也管到命令，完全放开也得进一个只藏数据根、别处都能写的沙盒。Linux 上 Landlock 挖不掉能写的里面的一处（施工 5-14 起照 dsh 不挖洞），得另想办法（例如密钥不放在家目录里能写的地方、或者完全放开也用 Landlock 只给能写的放开到数据根以外的几处）。推荐另开一步设计。
2. 内核的 `rule`、`session`、`workspace`、`grants`、`no_rule`：现在没人提规则，要不要连协议一起拆。推荐留着：是链的通用写法，拆了以后要让扩展守卫提规则还得加回来；头那边照有没有 `rule` 画，不用改。
3. `outbound`（对外发消息，例如本机会话里叫她主动给人发消息）只读时照旧问人：算不算「写」。推荐照旧问（发出去收不回，和改文件一类）。
