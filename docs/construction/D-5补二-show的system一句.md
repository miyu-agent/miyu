## 施工单 D-5 补二：show 的 system 一句

状态：核心的分会话做。D-5 补的验收第 4 条：改了说明以后实测，她还是有时不主动用 `show`；项目主人 2026-10-24 要在 system 里加一句再对照，对照完定留，做成开关、默认开。

### 为什么

`show` 的说明写上视频、音频以后，下了视频、生成了报表这类场景她还是常常只在正文里给一个路径，人得自己去打开。

### 两轮实测

同一个脚本：临时核心、开发端点的 `deepseek-v4.1-flash`，四个场景各 3 遍、两边交替，记她这一轮调没调 `show`。场景：用 python 画正弦曲线存 png、下载一个小的示例 mp4、写一个介绍猫的 HTML 页面存成文件、生成一份十几行的月度销售 csv。

| 场景 | 第一轮 改前说明 | 第一轮 改后说明 | 第二轮 改后说明、不带这一句 | 第二轮 改后说明、带这一句 |
|---|---|---|---|---|
| 画图 | 3/3 | 2/3 | 2/3 | 3/3 |
| 下载视频 | 0/3 | 2/3 | 2/3 | 3/3 |
| 网页 | 3/3 | 3/3 | 3/3 | 3/3 |
| 报表 | 2/3 | 3/3 | 1/3 | 3/3 |
| 合计 | 8/12 | 10/12 | 8/12 | 12/12 |

第二轮两边是同一个核心程序，只差资源里这一句（不带的那边是空文件），开会话核对过两边 system 只差 `system.show` 那一段。

结论：留，做成开关、默认开（项目主人 2026-10-24 定）。

### 做了什么

- `resources/software/basesystem/show/system.txt`：一句英文，常量字节，不拼会变的东西。登记簿 `26-提示词.md` 加一行，`prompts.md` 重新生成。
- `CoreLines.show` + `Snapshot::with_show`（`miyu-policy` 的 `compose.rs`）：工具面上有 `show` 的才带，接在调工具的程序那一段后面单成一段。
- 开关 `basesystem.show.system_hint`（`miyu-session` 的 `ShowSettings`）：开关，默认 `true`，系统、个人两层，新会话生效；核心替基础系统声明（`miyu-core` 的 `OWNED`），设置页在「软件包」那一页、基础系统那一组。名字、说明三语进 `core/human`（内置包的清单不能写 `[settings]`，同人格记忆、联网）。
- 造会话时读一次（`open/face.rs`），钉进快照 `no_show_hint`（关着的才写，以前的快照字节不变）；换人格、换预设重拼照钉下的，会话中途改了配置 system 不变，前缀稳定。
- `session.context` 多一块 `system.show`（`SystemPart::Show`），三语标签。
- 图纸：`tools/show.md`、`config.md`、`packages.md`、`protocol.md`、`reference/sessions(.en).md`；配置样本重新生成。

### 不做的

- 开关当场生效：system 中途变了整个前缀的缓存都断。

### 验收

1. `compose/tests.rs`：有 `show` 的单成一段，没有的 system 一字不变。
2. 会话层（`tests/tools.rs`）：有 `show` 的带、没有的不带；开关关了不带、开着带。
3. `preset_tests.rs`：关着造的会话换了预设重拼，配置这时开着也不带；开着造的照带。
4. 手改成永远带、造会话不读配置、快照无视开关、重拼不照钉下的，各至少红一条。
5. `miyu-core` 的配置清单测试、样本逐字节一样；`cargo xtask check` 全过，三平台 CI。
6. 头（终端、网页）：软件包设置页基础系统那一组多一项「提醒 AI 用展示」，默认开；关了以后新开的会话 system 里没有这一句（`session.context` 里没有 `system.show`）。
