## 施工单 WD-5：「社区软件包」四句、拒绝原因表、命令行文案和出厂软件包的字表清理

状态：做完了，等合并（分支 `step/WD-5-拒绝原因和命令行文案清理`，从 main 的 4524aa837 开）。主会话 2026-11-21 派，接在 WD-4 后面。

### 为什么

WD-4 清理了核心的字表 `resources/core/human`，留下四块没有处理：

1. 「社区包」四句提示（`packages/notice/*`）是项目主人 2026-11-12 定的原话，WD-4 没有修改。项目主人 2026-11-21 同意改成「软件包」的说法。
2. 协议拒绝时给人看的那句话写在代码里（`crates/miyu-endpoint/src/refusal/message.rs`，一个原因码一句中文、一句英文），里面有「登录没成。」「这个服务没登录过。」「软件源连不上。」这样的口语。
3. 命令行自己的文案也写在代码里（`crates/miyu-cli/src/language*`），有「← 不算：还没信任」「会删掉 {path}」「这份 Miyu 由 {manager} 安装，用包管理器升级」这样的说法。
4. 七个出厂软件包各有一张自己的字表（`resources/software/<包>/human`），工具的结果句几乎都是口语：「没有这个文件」「改了 {count} 处」「挪不进回收站」「此会话不支持长期目标」。拒绝原因表改了以后，同一次拒绝里 `message` 和 `data.why` 会一句新说法、一句旧说法，所以一起清理（主会话同意）。

这一步按 WD-4 的规矩（`internal/blueprint/config.md`「用词的规矩」）把这四块逐句过一遍。

### 改成什么样

- **「社区软件包」四句**（项目主人定的原话）：「这是社区软件包，未经官方审核，可能存在风险。」「这个软件包来自本机文件，未经官方审核，可能存在风险。」带包名的两句同理。英文、日文原来的意思没有变化，没有修改。命令行里同类的「（社区，未经审核）」「社区，没有经过官方审核」统一成「未经官方审核」。
- **拒绝原因表**：「没有这个…」统一成「…不存在。」，「…已经有了」统一成「…已存在。」，「…连不上／挂不上／挪不进」统一成「无法…」，「登记」写成「注册」。MCP 登录的四句都写明是「这个 MCP 服务」。
- **命令行**：报错仍然是 `miyu: <出了什么事>` 一行、`提示：<怎么办>` 一行。`config explain` 的记号改成「← 未生效：未信任／比下面几层更宽松／不能在这一层设置」「← 生效，仅限本次启动」；卸载确认屏改成「将删除 {path}」「配置保留在 {path}」；升级确认屏的「新要：」改成「新增：」。
- **出厂软件包的结果句**：状态变化的结果写「已…」（已安装、已移入回收站、已送达、已派出），查看类的结果写「动词 + 了」（读取了、列出了、编辑了 {count} 处），和核心字表 `view/summary/*` 的写法一致；报错写「文件不存在」「参数错误：{error}」「无法读取：{error}」「未找到」。
- **连接层的报错**（`crates/miyu-ipc/src`）是核心字表 `start/*` 的一份副本，字表没有读到时使用。它和 WD-4 改过的 `start/*` 对齐，`核心没有在运行` 三处（连接层、`miyu core status`、帮助页）统一成「核心未运行」。
- **命令行帮助页**（`crates/miyu-cli/src/help/zh`）改了 15 行里的口语（「照常结束」「别的原样留着」「已经在跑的核心」「只印网址」「取消登记」），每行不超过 80 列。

### 数量

| 部分 | 中文 | 英文 | 日文 |
|---|---|---|---|
| 核心字表（「社区软件包」四句） | 4 | 0 | 0 |
| 七个出厂软件包的字表 | 162 | 0 | 2 |
| 拒绝原因表 | 46 | 12 | 0 |
| 连接层的报错 | 8 | 0 | 0 |
| 命令行文案 | 80 | 0 | 0 |
| 命令行帮助页 | 15 行 | 0 | 0 |

字表里的键、代码里的原因码、占位符（不带名字的 `{}` 先后不变）、首尾空白、末尾标点都没有变化，由 `human_strings.py check`、`rust_strings.py check` 逐句校验。代码里只替换了字符串字面量，没有改动逻辑。

### 回代码核对后改正的事实

| 位置 | 原来的说法 | 实际行为和出处 |
|---|---|---|
| 拒绝原因 `embedding_off` | 「语义模型没有开。」 | 除了 `models.embedding` 关闭，语义模型没有安装、文件缺失、连续无法启动、远程供应商没有配置时也返回它（`crates/miyu-endpoint/src/embedding.rs` 的 `embed`，`crates/miyu-session/src/embed.rs` 的 `Unavailable::Off`）。改成「语义模型已关闭或无法使用。」 |
| 拒绝原因 `password_not_set` | 「…请在这台电脑上打开后设置。」 | `crates/miyu-endpoint/src/hello.rs` 只对远程来的连接返回这一句，读到它的人在另一台设备上。改成「运行 Miyu 的电脑」 |
| 拒绝原因 `unknown_watch` | 「这边没有在等这个通知。」 | 拒绝回给发通知的会话，没有在等的是收通知的那个会话（`crates/miyu-kernel/src/session/peers.rs`）。改成「对方没有在等待这个通知。」 |
| 拒绝原因的英文四句 | `package_depended`、`already_heard` 少了「无法卸载」「无法撤回」半句；`session_read_only` 的 `before it` 没有所指；`not_logged_in` 写成 `This server is not logged in.` | 照中文补全 |
| 命令行 `--prerelease` 的提示 | 「加 '--prerelease' 可以看预发布」 | 这个选项让预发布版成为升级的候选（`crates/miyu-cli/src/upgrade/flow.rs`），改成「可以升级到预发布版」 |
| 命令行找不到 litemiyu 的提示 | 「安装：Arch Linux 用 AUR…」，拼出来是「提示：安装：…」 | 改成「在 Arch Linux 上从 AUR 安装…」。提示开头不能是首字母大写的英文词：`crates/miyu-cli/src/diagnostic.rs` 的 `tidy` 会把它改成小写 |
| 基础系统 `said/package/failed`（中文、日文） | 「没装成、没卸成」 | 安装、从软件源安装、升级、卸载、启用五种失败和参数不对都发这一句（`crates/miyu-basesystem/src/package.rs`），改成「软件包操作失败」 |

### 没有修改的，和原因

- **英文那一份里会进给模型的句子**：出厂软件包的结果句会写进会话导出，`agent` 用途的导出一律使用英文那一份。子代理提出的四处英文改动（`package/failed`、知识库 `common/off`、`send_message/not-yours`、`common/too-large`）没有采纳；`export/*` 的三句英文同 WD-4，没有修改。
- **「模型驱动 {driver} 会让供应商自己运行：{names}」**没有修改：网页界面有同一句的副本，手册写明两处是同一句。
- **功能的名字**没有修改：「装回」「自动想起」「后台页」「小程序」是手册和界面通用的叫法。工具的显示名（「找文件」「搜内容」「翻记录」「查用量」）偏口语，但是手册的表和各界面都逐字使用，要不要改由项目主人决定。
- **三句拒绝原因**没有找到一句能覆盖所有场合的说法，保持原样：`local_only`「仅限本机。」（扩展调用只给人用的方法时也返回它）、`unknown_server`「设置里没有这个 MCP 服务。」、`command_not_allowed`「仅主人和群管理员可用。」（命令停用时也返回它，另带 `data.why`）。
- **不是文案的中文**没有修改：`text_rules.rs` 的测试代码、属性里的理由（`#[expect(..., reason = "…")]`）、`unreachable!` 的话。

### 连带的修改

- **测试**：45 个测试文件里逐字断言的旧话换成新话，断言的写法没有变化。
- **新测试**：`crates/miyu-core/tests/human_wording.rs` 加一条 `shipped_package_words_avoid_construction_slang`，七个出厂软件包的中文字表不出现 WD-4 那张词表里的字眼；基点上有 6 句会被它拦住。
- **样本**：`internal/designs/samples/cli/` 的三份（`ask-text.txt`、`ask-agents-text.txt`、`ask-agents-messaged-text.txt`）手工改了 7 行；蓝图里对应的样本块和两页帮助（`ask.txt`、`kb.txt`）的样本块同步。
- **蓝图**：28 页。表格里照抄句子的 122 格换成新话（各工具的结果句、拒绝原因的中英对照表、命令行各子命令的字）；「用词的规矩」补两条。行文里引用的旧话是当时的记录，没有修改。
- **手册和协议参考**：中文 17 页、英文 2 页里写在反引号里的原话换成新话。

### 真跑的结果

在 .91 上用沙箱数据根（`HOME`、`MIYU_HOME` 都指向临时目录）编好以后实际运行，看到的输出：

```
$ miyu core status
核心未运行
提示：运行 'miyu core start' 启动

$ miyu config explain permission.start_read_only        （项目配置没有信任）
  true   项目配置  …/proj/.miyu/config.toml:2  ← 未生效：未信任
  false  默认值    ← 生效

$ miyu config explain log.level                         （写在项目配置里）
  "debug"  项目配置  …/proj/.miyu/config.toml:4  ← 未生效：不能在这一层设置

$ miyu config path --project
miyu: 警告：文件还不存在

$ miyu kb new notes ; miyu kb new notes
已创建 notes
miyu: 知识库已存在

$ miyu kb mount ext /不存在的目录
miyu: 无法挂载这个目录：mounted folder …: No such file or directory (os error 2)

$ miyu kb add notes f.md （第二次）
miyu: 文档已存在

$ miyu kb read notes f.md --page 9
miyu: 第 9 页不存在：共 1 页

$ miyu schedule pause t1
已暂停 t1  every 10m  已暂停  新会话 无人格 预设 full …

$ miyu schedule rm t99
miyu: 定时任务不存在

$ miyu mcp logout nosuch
miyu: 还没有登录这个 MCP 服务

$ miyu pkg -Qi ./somewhere
miyu: -Qi 只接受软件包编号

$ miyu rescue
miyu: 未找到 litemiyu
提示：在 Arch Linux 上从 AUR 安装（paru -S litemiyu 或 yay -S litemiyu）；其他发行版从 … 下载

$ miyu upgrade
当前的 Miyu 不是用安装脚本或包管理器安装的，不自动升级
提示：从 https://github.com/miyu-agent/miyu/releases/latest 获取新版
```

帮助页 `miyu core --help`、`miyu dev --help`、`miyu bus --help` 的新措辞和换行也看过。没有实际触发的：升级确认屏（需要软件源）、卸载确认屏的「将删除」「配置保留在」（出厂软件包没有状态目录）、`miyu ask` 的步骤行（需要模型），这几处由测试覆盖。

### 真跑时看到、不属于这一步的问题

- `miyu pkg pack <目录>` 在中文环境下输出英文的原因：`miyu: 无法打包：line 2: package.id is not a known key`；同一个目录用 `miyu pkg info` 输出的是中文「未知的键：package.id」。
- `miyu kb mount` 失败时在中文句子后面接了英文的系统原因：`无法挂载这个目录：mounted folder …: No such file or directory (os error 2)`。
- 读代码看到、没有运行：长期目标的 `said/goal/failed` 里 `{reason}` 填的是英文原因码，人会看到「目标未更改：goal_paused」；循环的 `said/next/set`「下次 {next}」在没有下一次时间时显示成「下次 」；`said/todowrite/cleared`「待办已全部完成，已清空」在写入空列表时也会显示。

### 留给其他会话的

- **终端界面、网页界面、通讯平台的仓库**断言或抄写了这些话的地方，换钉时按下面的对照表修改。已知的副本：终端界面和网页界面各有一份「文件已变」「文件已不在」（核心的拒绝原因现在是「文件已更改。」「文件已不存在。」）；各界面的测试里钉着的工具结果句（例如「没有这个文件」「改了 1 处」）。
- **`packaging/install.sh`** 里和 `miyu upgrade` 对应的几句（「连不上 GitHub」「GitHub 限流了…过一会儿再试」「GitHub 回了」）没有在这一步修改。
- **软件包清单里的名字**（`miyu pkg list` 显示的「找文件」「搜内容」「翻查本会话」）不在字表里，没有修改。

### 验收

- `human_strings.py check`、`rust_strings.py check`：占位符、首尾空白、末尾标点逐句不变。
- 全仓 `cargo test --workspace` 通过；门禁十二项通过；三个平台的 CI 通过。
- `python3 ~/.cache/miyu-docs-2026-11-14/lang/check.py .`：中英各 62 页，问题 0 个。
- 手册里引用的界面原话用 `uiscan.py` 和基点对比，新增的对不上的引用只有把占位符填成例子的几句，逐句核对过。

### 对照表

一共 314 处（帮助页的 15 行另见提交的差异）。字表里的按键列出，写在代码里的按文件列出；代码里的句子没有键，`{…}` 是 Rust 的占位符。

#### 核心字表 `resources/core/human`（「社区软件包」四句）

共 4 处。

| 键 | 语言 | 原来 | 现在 |
|---|---|---|---|
| `said/packages/notice/unreviewed` | zh | `这是个社区包，没有经过官方审核，可能存在风险。` | `这是社区软件包，未经官方审核，可能存在风险。` |
| `said/packages/notice/unreviewed/named` | zh | `{packages} 是社区包，没有经过官方审核，可能存在风险。` | `{packages} 是社区软件包，未经官方审核，可能存在风险。` |
| `said/packages/notice/file` | zh | `这个包来自本机文件，没有经过官方审核，可能存在风险。` | `这个软件包来自本机文件，未经官方审核，可能存在风险。` |
| `said/packages/notice/file/named` | zh | `{packages} 来自本机文件，没有经过官方审核，可能存在风险。` | `{packages} 来自本机文件，未经官方审核，可能存在风险。` |

#### 出厂软件包 basesystem（`resources/software/basesystem/human`）

共 101 处。

| 键 | 语言 | 原来 | 现在 |
|---|---|---|---|
| `said/read/empty` | zh | `空的` | `内容为空` |
| `said/read/past-end` | zh | `过了结尾，一共 {total} 行` | `超出末尾，一共 {total} 行` |
| `said/read/past-end-entries` | zh | `过了结尾，一共 {total} 项` | `超出末尾，一共 {total} 项` |
| `said/read/binary` | zh | `是二进制文件，没读` | `二进制文件，未读取` |
| `said/read/image-scaled` | zh | `图片 {width}×{height}（缩小过）` | `图片 {width}×{height}（已缩小）` |
| `said/read/image-too-big` | zh | `图太大（{size}），没读` | `图片太大（{size}），未读取` |
| `said/read/image-too-wide` | zh | `图太大（{width}×{height}），没读` | `图片太大（{width}×{height}），未读取` |
| `said/common/missing` | zh | `没有这个文件` | `文件不存在` |
| `said/common/missing-similar` | zh | `没有这个文件，是不是 {similar}` | `文件不存在，相近的文件：{similar}` |
| `said/common/failed` | zh | `读不了：{error}` | `无法读取：{error}` |
| `said/common/bad-args` | zh | `参数不对：{error}` | `参数错误：{error}` |
| `said/common/bad-glob` | zh | `通配写得不对：{error}` | `通配符格式不正确：{error}` |
| `said/common/no-files` | zh | `一个都没找到` | `未找到文件` |
| `said/glob/files-more` | zh | `{total} 个文件，列了 {shown} 个` | `{total} 个文件，已列出 {shown} 个` |
| `said/grep/matches-more` | zh | `第 {from}–{to} 处，后面还有` | `第 {from}–{to} 处，后面还有更多` |
| `said/grep/none` | zh | `没搜到` | `未找到` |
| `said/grep/past-end` | zh | `跳过了全部，一共 {total} 条` | `已跳过全部结果，一共 {total} 条` |
| `said/grep/bad-pattern` | zh | `正则写得不对：{error}` | `正则表达式格式不正确：{error}` |
| `said/common/not-read` | zh | `没读过，要先读` | `还没有读取，需要先读取` |
| `said/common/stale` | zh | `读过以后又改过了，要重读` | `读取之后文件又被修改，需要重新读取` |
| `said/common/directory` | zh | `是个目录` | `是目录` |
| `said/common/write-failed` | zh | `写不了：{error}` | `无法写入：{error}` |
| `said/edit/edited` | zh | `改了 {count} 处` | `编辑了 {count} 处` |
| `said/edit/no-edits` | zh | `没给要改的` | `未指定要修改的内容` |
| `said/edit/empty` | zh | `第 {index} 处的原文是空的` | `第 {index} 处的原文为空` |
| `said/edit/same` | zh | `第 {index} 处改前改后一样` | `第 {index} 处修改前后相同` |
| `said/edit/not-found` | zh | `第 {index} 处没找到` | `第 {index} 处未找到` |
| `said/edit/not-found-near` | zh | `第 {index} 处没找到，最像的在第 {line} 行` | `第 {index} 处未找到，最接近的在第 {line} 行` |
| `said/edit/not-unique` | zh | `第 {index} 处对得上 {count} 个地方` | `第 {index} 处匹配到 {count} 个位置` |
| `said/edit/overlap` | zh | `第 {first} 处和第 {second} 处重叠了` | `第 {first} 处和第 {second} 处重叠` |
| `said/trash/trashed` | zh | `移进了回收站` | `已移入回收站` |
| `said/trash/unavailable` | zh | `放不进回收站，没删` | `无法移入回收站，未删除` |
| `said/trash/protected` | zh | `工作目录、它的上级、家目录、根目录不能删` | `工作目录、它的上级目录、家目录、根目录不能删除` |
| `said/trash/lost` | zh | `删了，可回收站里找不到` | `已删除，但在回收站里找不到` |
| `said/trash/failed` | zh | `删不了：{error}` | `无法删除：{error}` |
| `said/show/shown` | zh | `展示给了用户` | `已展示给用户` |
| `said/shell/signal` | zh | `被信号 {signal} 停掉了` | `被信号 {signal} 终止` |
| `said/shell/timed-out` | zh | `超过 {seconds} 秒，停掉了` | `超过 {seconds} 秒，已停止` |
| `said/shell/failed` | zh | `执行不了：{error}` | `无法执行：{error}` |
| `said/shell/no-background` | zh | `这里不能放到后台跑` | `这里不能放到后台运行` |
| `said/shell/no-sleep` | zh | `不许前台用 sleep 干等` | `不允许在前台用 sleep 等待` |
| `said/shell/no-cwd` | zh | `工作目录不存在了：{cwd}` | `工作目录已不存在：{cwd}` |
| `said/shell/hazard-delete_root` | zh | `没有执行：会删掉根目录` | `未执行：会删除根目录` |
| `said/shell/hazard-delete_home` | zh | `没有执行：会删掉家目录` | `未执行：会删除家目录` |
| `said/shell/hazard-delete_system` | zh | `没有执行：会删掉系统目录` | `未执行：会删除系统目录` |
| `said/shell/hazard-disk_format` | zh | `没有执行：会格式化磁盘` | `未执行：会格式化磁盘` |
| `said/shell/hazard-disk_write` | zh | `没有执行：会直接写磁盘设备` | `未执行：会直接写入磁盘设备` |
| `said/shell/hazard-container_host` | zh | `没有执行：会让容器拿到整台机器的设备` | `未执行：会让容器访问整台机器的设备` |
| `said/shell/background` | zh | `放到后台了：{job}` | `已放到后台运行：{job}` |
| `said/shell/moved` | zh | `超时，挪到后台了：{job}` | `超时，已转到后台运行：{job}` |
| `said/history/read` | zh | `读了第 {from}–{to} 条` | `读取了第 {from}–{to} 条` |
| `said/history/none` | zh | `没有找到` | `未找到` |
| `said/history/no-log` | zh | `读不了记录：{error}` | `无法读取记录：{error}` |
| `said/history/no-session` | zh | `没有会话 {session}` | `会话 {session} 不存在` |
| `said/history/ambiguous` | zh | `{session} 对得上不止一个会话` | `{session} 匹配到多个会话` |
| `said/history/not-here` | zh | `这里不能读别的会话` | `这里不能读取其他会话` |
| `said/agent/started` | zh | `派出去了：{job}` | `已派出：{job}` |
| `said/agent/not-started` | zh | `派不出去` | `无法派出子代理` |
| `said/agent/no-fork` | zh | `分叉出来的不能再分叉` | `分叉出的子代理不能再分叉` |
| `said/advisor/answered` | zh | `答了` | `已回答` |
| `said/advisor/failed` | zh | `没答成：{reason}` | `回答失败：{reason}` |
| `said/jobs/output` | zh | `读了 {job} 的输出` | `读取了 {job} 的输出` |
| `said/jobs/stopped` | zh | `停掉了 {job}` | `已停止 {job}` |
| `said/jobs/unknown` | zh | `没有任务 {id}` | `任务 {id} 不存在` |
| `said/jobs/ended` | zh | `{job} 已经结束了` | `{job} 已结束` |
| `said/send_message/sent` | zh | `送到了：{to}` | `已送达：{to}` |
| `said/send_message/held` | zh | `存下了：{to} 是没人看着的一次性会话` | `已保存：{to} 是没人看着的一次性会话` |
| `said/send_message/not-yours` | zh | `{to} 不是它派的子代理` | `{to} 不是这个会话派出的子代理` |
| `said/send_message/stopped` | zh | `{to} 已经停了` | `{to} 已停止` |
| `said/send_message/not-sent` | zh | `没送到` | `未送达` |
| `said/send_message/no-session` | zh | `没有会话 {to}` | `会话 {to} 不存在` |
| `said/send_message/ambiguous` | zh | `{to} 对得上不止一个会话` | `{to} 匹配到多个会话` |
| `said/send_message/not-here` | zh | `这里不能给别的会话发话` | `这里不能给其他会话发消息` |
| `said/send_message/too-long` | zh | `太长了：{chars} 个字，最多 {limit}` | `消息过长：{chars} 个字，最多 {limit} 个字` |
| `said/send_message/too-many` | zh | `刚给 {to} 发得太多了` | `短时间内发给 {to} 的消息过多` |
| `said/send_message/duplicate` | zh | `{to} 刚收到过一样的话` | `{to} 刚刚收到过相同的消息` |
| `said/send_message/inbox-full` | zh | `{to} 没看的话太多了` | `{to} 的未读消息已达上限` |
| `said/send_message/watching` | zh | `{to} 空下来时告诉 AI` | `{to} 空闲时会通知 AI` |
| `said/send_message/watch-peers-only` | zh | `只能等别的会话空下来` | `只能等待其他会话空闲` |
| `said/attachments/too-many` | zh | `最多带 {limit} 个附件` | `最多添加 {limit} 个附件` |
| `said/attachments/too-big` | zh | `{path} 太大，带不了` | `{path} 太大，无法作为附件` |
| `said/sessions/none` | zh | `没有别的会话` | `没有其他会话` |
| `said/sessions/past-end` | zh | `一共 {total} 个别的会话，已经列完了` | `一共 {total} 个其他会话，已全部列出` |
| `said/sessions/failed` | zh | `没列出来：{error}` | `无法列出会话：{error}` |
| `said/session_usage/shown` | zh | `查了用量` | `查看了用量` |
| `said/session_usage/failed` | zh | `用量读不出来：{error}` | `无法读取用量：{error}` |
| `said/ask_user/answered` | zh | `你答了` | `你已回答` |
| `said/ask_user/unattended` | zh | `这里没人能回答` | `这里没有人可以回答` |
| `said/todowrite/updated` | zh | `更新了待办：做完 {done}/{total}` | `更新了待办：已完成 {done}/{total}` |
| `said/todowrite/cleared` | zh | `待办都做完了，清空` | `待办已全部完成，已清空` |
| `said/packages/listed` | zh | `列了软件包` | `列出了软件包` |
| `said/packages/shown` | zh | `看了 {package}` | `查看了 {package}` |
| `said/packages/failed` | zh | `没看成软件包` | `无法查看软件包` |
| `said/package/installed` | zh | `装了 {package}` | `已安装 {package}` |
| `said/package/removed` | zh | `卸了 {package}` | `已卸载 {package}` |
| `said/package/enabled` | zh | `打开了 {package}` | `已启用 {package}` |
| `said/package/failed` | zh | `没装成、没卸成` | `软件包操作失败` |
| `said/packages/searched` | zh | `在软件源里搜了 {query}` | `在软件源里搜索了 {query}` |
| `said/package/upgraded` | zh | `升级了 {packages}` | `已升级 {packages}` |
| `said/package/nothing-to-upgrade` | zh | `没有要升级的` | `没有需要升级的软件包` |
| `said/package/failed` | ja | `インストールも削除もできませんでした` | `パッケージの操作に失敗しました` |

#### 出厂软件包 knowledge（`resources/software/knowledge/human`）

共 21 处。

| 键 | 语言 | 原来 | 现在 |
|---|---|---|---|
| `said/kb_search/nothing` | zh | `没找到` | `未找到` |
| `said/common/bad-args` | zh | `参数不对：{error}` | `参数错误：{error}` |
| `said/common/off` | zh | `这个会话查不了知识库` | `这个会话不能使用知识库` |
| `said/common/failed` | zh | `知识库现在用不了：{error}` | `知识库暂时不可用：{error}` |
| `said/common/no-base` | zh | `没有知识库 {kb}` | `知识库 {kb} 不存在` |
| `said/common/no-document` | zh | `{kb} 里没有 {path}` | `{kb} 里的文档 {path} 不存在` |
| `said/common/no-page` | zh | `没有第 {page} 页` | `第 {page} 页不存在` |
| `said/kb_save/saved` | zh | `记进了 {kb} 的 {path}` | `已保存到 {kb} 的 {path}` |
| `said/kb_save/appended` | zh | `接在 {kb} 的 {path} 后面` | `已追加到 {kb} 的 {path}` |
| `said/kb_save/created` | zh | `建了知识库 {kb}，记进 {path}` | `已创建知识库 {kb}，并保存到 {path}` |
| `said/kb_edit/edited` | zh | `改了 {kb} 的 {path}` | `已编辑 {kb} 的 {path}` |
| `said/kb_delete/trashed` | zh | `把 {kb} 的 {path} 挪进了回收站` | `已把 {kb} 的 {path} 移入回收站` |
| `said/common/exists` | zh | `{kb} 里已经有 {path}` | `{kb} 里的文档 {path} 已存在` |
| `said/common/too-large` | zh | `{path} 太大了` | `{path} 太大` |
| `said/common/no-match` | zh | `{path} 里找不到要换的字` | `在 {path} 里未找到要替换的文本` |
| `said/common/ambiguous` | zh | `{path} 里要换的字有 {count} 处` | `{path} 里要替换的文本有 {count} 处` |
| `said/common/trash` | zh | `{path} 挪不进回收站：{error}` | `无法把 {path} 移入回收站：{error}` |
| `said/common/bad-id` | zh | `{kb} 不能当知识库的编号` | `{kb} 不能用作知识库的编号` |
| `said/common/read-only` | zh | `{kb} 是挂进来的目录，只读` | `{kb} 是挂载的目录，只读` |
| `said/common/packaged` | zh | `{kb} 随软件包来，只读` | `{kb} 随软件包提供，只读` |
| `said/common/not-text` | zh | `{path} 不是文字的文档，写不了` | `{path} 不是文本文档，无法写入` |

#### 出厂软件包 net（`resources/software/net/human`）

共 17 处。

| 键 | 语言 | 原来 | 现在 |
|---|---|---|---|
| `said/web_fetch/fetched` | zh | `抓到了` | `已抓取` |
| `said/web_fetch/bad-args` | zh | `参数不对：{error}` | `参数错误：{error}` |
| `said/web_fetch/blocked` | zh | `{url} 是本机或内网的地址，抓不了` | `{url} 是本机或内网地址，无法抓取` |
| `said/web_fetch/unreachable` | zh | `连不上 {url}` | `无法连接 {url}` |
| `said/web_fetch/redirects` | zh | `{url} 跳转太多次` | `{url} 跳转次数过多` |
| `said/web_fetch/challenge` | zh | `{url} 回的是人机验证页` | `{url} 返回的是人机验证页` |
| `said/web_fetch/status` | zh | `{url} 回了 HTTP {status}` | `{url} 返回 HTTP {status}` |
| `said/web_fetch/unsupported` | zh | `{url} 是 {type}，读不成字` | `{url} 是 {type}，无法读取为文本` |
| `said/web_fetch/too-complex` | zh | `{url} 太大或嵌套太深，换不成字` | `{url} 太大或嵌套太深，无法转换为文本` |
| `said/web_fetch/empty` | zh | `{url} 没有能读的字` | `{url} 没有可以读取的文本` |
| `said/web_fetch/not-ready` | zh | `现在抓不了网页` | `暂时无法抓取网页` |
| `said/web_fetch/no-page` | zh | `没有第 {page} 页，一共 {pages} 页` | `第 {page} 页不存在，一共 {pages} 页` |
| `said/web_search/nothing` | zh | `没搜到` | `未找到` |
| `said/web_search/bad-args` | zh | `参数不对：{error}` | `参数错误：{error}` |
| `said/web_search/failed` | zh | `搜不了：{why}` | `搜索失败：{why}` |
| `said/web_search/cooling` | zh | `几家搜索都被限额了，过会儿再试` | `所有搜索服务都被限额，稍后重试` |
| `said/web_search/not-ready` | zh | `现在搜不了` | `暂时无法搜索` |

#### 出厂软件包 memory（`resources/software/memory/human`）

共 13 处。

| 键 | 语言 | 原来 | 现在 |
|---|---|---|---|
| `said/remember/saved` | zh | `记成 {id}` | `已保存为 {id}` |
| `said/remember/replaced` | zh | `记成 {id}，替换 {old}` | `已保存为 {id}，替换了 {old}` |
| `said/remember/too-long` | zh | `太长，{chars} 个字` | `记忆过长：{chars} 个字` |
| `said/remember/unknown-class` | zh | `不认识的类 {class}` | `未知的类别：{class}` |
| `said/remember/no-who` | zh | `认识的人要写是谁` | `「认识的人」类别的记忆需要写明是谁` |
| `said/memory_search/found-cards` | zh | `找到 {cards} 张人卡、{memories} 条记忆、{turns} 段对话` | `找到 {cards} 张卡片、{memories} 条记忆、{turns} 段对话` |
| `said/memory_search/nothing` | zh | `没找到` | `未找到` |
| `said/common/bad-args` | zh | `参数不对：{error}` | `参数错误：{error}` |
| `said/common/off` | zh | `这个会话没开记忆` | `这个会话未启用记忆` |
| `said/common/failed` | zh | `记忆现在用不了：{error}` | `记忆暂时不可用：{error}` |
| `said/common/no-such` | zh | `没有 {id}` | `记忆 {id} 不存在` |
| `said/common/not-current` | zh | `{id} 已经改掉或忘掉了` | `{id} 已被替换或已忘掉` |
| `said/remember/unknown-class` | ja | `知らない分類 {class}` | `不明な分類：{class}` |

#### 出厂软件包 schedule（`resources/software/schedule/human`）

共 6 处。

| 键 | 语言 | 原来 | 现在 |
|---|---|---|---|
| `said/cron/created` | zh | `已建 {id}` | `已创建 {id}` |
| `said/cron/full` | zh | `此会话已有 {limit} 个定时任务` | `这个会话已有 {limit} 个定时任务` |
| `said/cron/bad-cron` | zh | `cron 写法不对：{error}` | `cron 表达式格式不正确：{error}` |
| `said/cron/not-found` | zh | `没有 {id}` | `定时任务 {id} 不存在` |
| `said/common/bad-args` | zh | `参数不对：{error}` | `参数错误：{error}` |
| `said/common/off` | zh | `此会话不支持定时任务` | `这个会话不支持定时任务` |

#### 出厂软件包 goal（`resources/software/goal/human`）

共 2 处。

| 键 | 语言 | 原来 | 现在 |
|---|---|---|---|
| `said/common/bad-args` | zh | `参数不对：{error}` | `参数错误：{error}` |
| `said/common/off` | zh | `此会话不支持长期目标` | `这个会话不支持长期目标` |

#### 出厂软件包 loop（`resources/software/loop/human`）

共 4 处。

| 键 | 语言 | 原来 | 现在 |
|---|---|---|---|
| `said/next/no-loop` | zh | `没有在跑的循环` | `没有运行中的循环` |
| `said/next/no-delay` | zh | `没给间隔` | `未指定间隔` |
| `said/common/bad-args` | zh | `参数不对：{error}` | `参数错误：{error}` |
| `said/common/off` | zh | `此会话不支持循环` | `这个会话不支持循环` |

#### 拒绝原因表 `crates/miyu-endpoint/src/refusal/message.rs`

共 58 处。

| 文件 | 原来 | 现在 |
|---|---|---|
| `miyu-endpoint/src/refusal/message.rs` | `会话日志第 {line} 行坏了，只能看到这一行以前的内容，不能接着聊。要继续请新开一个会话。` | `会话日志第 {line} 行已损坏，只能查看这一行之前的内容，不能继续对话。如果要继续，请新建一个会话。` |
| `miyu-endpoint/src/refusal/message.rs` | `消息超过 1 MiB，没有收。大的内容请用附件发。` | `消息超过 1 MiB，未接收。较大的内容请用附件发送。` |
| `miyu-endpoint/src/refusal/message.rs` | `同一个请求编号换了参数，没有收。请换一个新的编号。` | `这个请求编号已经用于参数不同的请求，未接收。请换一个新的编号。` |
| `miyu-endpoint/src/refusal/message.rs` | `没有这个软件源。` | `软件源不存在。` |
| `miyu-endpoint/src/refusal/message.rs` | `Cannot write there.` | `Cannot write to this location.` |
| `miyu-endpoint/src/refusal/message.rs` | `无法写入该位置。` | `无法写入这个位置。` |
| `miyu-endpoint/src/refusal/message.rs` | `导出超过 512 KiB，请存成文件。` | `导出内容超过 512 KiB，请保存到文件。` |
| `miyu-endpoint/src/refusal/message.rs` | `这个服务要先在它那边注册客户端。` | `登录这个 MCP 服务之前，需要先在服务提供方注册客户端。` |
| `miyu-endpoint/src/refusal/message.rs` | `登录没成。` | `登录失败。` |
| `miyu-endpoint/src/refusal/message.rs` | `This server is not logged in.` | `Not logged in to this MCP server.` |
| `miyu-endpoint/src/refusal/message.rs` | `这个服务没登录过。` | `还没有登录这个 MCP 服务。` |
| `miyu-endpoint/src/refusal/message.rs` | `这个服务没有在等的登录，先发起登录。` | `这个 MCP 服务没有等待完成的登录，需要先发起登录。` |
| `miyu-endpoint/src/refusal/message.rs` | `No login is waiting for this server; start one first.` | `No login is pending for this server; start one first.` |
| `miyu-endpoint/src/refusal/message.rs` | `MCP 服务写法有误。` | `MCP 服务格式不正确。` |
| `miyu-endpoint/src/refusal/message.rs` | `这个技能在别的目录里，别的应用也在用，请到那里删除。` | `这个技能位于其他技能目录，其他应用也在使用，请到那个目录中删除。` |
| `miyu-endpoint/src/refusal/message.rs` | `包文件无法解开。` | `无法解压软件包文件。` |
| `miyu-endpoint/src/refusal/message.rs` | `软件源连不上。` | `无法连接软件源。` |
| `miyu-endpoint/src/refusal/message.rs` | `Other packages depend on it.` | `Other packages depend on it, so it cannot be uninstalled.` |
| `miyu-endpoint/src/refusal/message.rs` | `有会话正在用它的模型驱动，等它们说完再查。` | `有会话正在使用它的模型驱动，等这些调用结束后再检查。` |
| `miyu-endpoint/src/refusal/message.rs` | `方法未登记。` | `方法未注册。` |
| `miyu-endpoint/src/refusal/message.rs` | `这个名字有几个软件包接着，要写用哪一个。` | `有多个软件包接入了这个名字，需要指定使用哪一个。` |
| `miyu-endpoint/src/refusal/message.rs` | `Several packages plug into this name; say which one.` | `Several packages plug into this name; specify which one.` |
| `miyu-endpoint/src/refusal/message.rs` | `扩展照包编号只能调自己，调别的软件包请用 bus.call。` | `扩展按软件包编号只能调用自己，调用其他软件包请使用 bus.call。` |
| `miyu-endpoint/src/refusal/message.rs` | `只有你自己能在这里跑命令。` | `只有你本人可以在这里运行命令。` |
| `miyu-endpoint/src/refusal/message.rs` | `没有能跑命令的工具（软件包没装或已关闭）。` | `没有可以运行命令的工具（软件包未安装或已停用）。` |
| `miyu-endpoint/src/refusal/message.rs` | `会话已停止，重新发送即可载入。` | `会话已停止，重新发送就可以载入。` |
| `miyu-endpoint/src/refusal/message.rs` | `会话日志坏了，只能看到坏的地方以前的内容，不能接着聊。要继续请新开一个会话。` | `会话日志已损坏，只能查看损坏位置之前的内容，不能继续对话。如果要继续，请新建一个会话。` |
| `miyu-endpoint/src/refusal/message.rs` | `The session log is broken; only the part before it can be viewed and the session can't continue. Start a new session to go on.` | `The session log is broken; only the part before the broken line can be viewed and the session can't continue. Start a new session to go on.` |
| `miyu-endpoint/src/refusal/message.rs` | `Message already delivered.` | `Message already delivered and cannot be withdrawn.` |
| `miyu-endpoint/src/refusal/message.rs` | `知识库已经有了。` | `知识库已存在。` |
| `miyu-endpoint/src/refusal/message.rs` | `这一篇已经有了。` | `文档已存在。` |
| `miyu-endpoint/src/refusal/message.rs` | `挂进来的知识库是只读的。` | `挂载的知识库是只读的。` |
| `miyu-endpoint/src/refusal/message.rs` | `随软件包来的知识库是只读的，只能开关自动想起。` | `随软件包提供的知识库是只读的，只能开启或关闭自动想起。` |
| `miyu-endpoint/src/refusal/message.rs` | `A knowledge base from a package is read-only; only auto recall can be switched.` | `A knowledge base from a package is read-only; only auto recall can be turned on or off.` |
| `miyu-endpoint/src/refusal/message.rs` | `这个目录挂不上。` | `无法挂载这个目录。` |
| `miyu-endpoint/src/refusal/message.rs` | `挪不进回收站。` | `无法移入回收站。` |
| `miyu-endpoint/src/refusal/message.rs` | `此会话不支持长期目标。` | `这个会话不支持长期目标。` |
| `miyu-endpoint/src/refusal/message.rs` | `回答与题目不匹配。` | `回答与提问不匹配。` |
| `miyu-endpoint/src/refusal/message.rs` | `还没有设访问密码，请在这台电脑上打开后设置。` | `还没有设置访问密码，请在运行 Miyu 的电脑上打开后设置。` |
| `miyu-endpoint/src/refusal/message.rs` | `No access password yet. Set one on this computer first.` | `No access password yet. Set one on the computer running Miyu first.` |
| `miyu-endpoint/src/refusal/message.rs` | `没有这个展示。` | `展示不存在。` |
| `miyu-endpoint/src/refusal/message.rs` | `文件已变。` | `文件已更改。` |
| `miyu-endpoint/src/refusal/message.rs` | `The file is gone.` | `The file no longer exists.` |
| `miyu-endpoint/src/refusal/message.rs` | `文件已不在。` | `文件已不存在。` |
| `miyu-endpoint/src/refusal/message.rs` | `只能在核心所在的机器上用。` | `只能在核心所在的机器上使用。` |
| `miyu-endpoint/src/refusal/message.rs` | `没有这个定时任务。` | `定时任务不存在。` |
| `miyu-endpoint/src/refusal/message.rs` | `定时任务已经有 200 个，先删掉几个再加。` | `定时任务已有 200 个，需要先删除几个才能添加。` |
| `miyu-endpoint/src/refusal/message.rs` | `The embedding model is off.` | `The embedding model is off or unavailable.` |
| `miyu-endpoint/src/refusal/message.rs` | `语义模型没有开。` | `语义模型已关闭或无法使用。` |
| `miyu-endpoint/src/refusal/message.rs` | `语义模型没算出来，详见运行日志。` | `语义模型计算失败，详见运行日志。` |
| `miyu-endpoint/src/refusal/message.rs` | `界面块的内容写法不对。` | `界面块的内容格式不正确。` |
| `miyu-endpoint/src/refusal/message.rs` | `没有这个权限级别。` | `权限级别不存在。` |
| `miyu-endpoint/src/refusal/message.rs` | `没有这个选项。` | `选项不存在。` |
| `miyu-endpoint/src/refusal/message.rs` | `这个会话发来的话太多了，过一会儿再发。` | `这个会话发来的消息过多，稍后再发送。` |
| `miyu-endpoint/src/refusal/message.rs` | `刚发过一字不差的一句，没有再收。` | `刚刚发送过完全相同的消息，未重复接收。` |
| `miyu-endpoint/src/refusal/message.rs` | `对方还没看的话已经够多了，过一会儿再发。` | `对方未读的消息已达上限，稍后再发送。` |
| `miyu-endpoint/src/refusal/message.rs` | `这边没有在等这个通知。` | `对方没有在等待这个通知。` |
| `miyu-endpoint/src/refusal/message.rs` | `Nothing here is waiting for this notice.` | `The other session is not waiting for this notice.` |

#### 连接层的报错 `crates/miyu-ipc/src`

共 8 处。

| 文件 | 原来 | 现在 |
|---|---|---|
| `miyu-ipc/src/error.rs` | `套接字的路径太长，放不下：{}；临时目录下也放不下` | `套接字路径过长：{}；改放到临时目录下也超出长度限制` |
| `miyu-ipc/src/error.rs` | `{} 不属于当前用户，或者其他用户也能访问：套接字不放在这里` | `{} 不属于当前用户，或者其他用户也能访问，因此不在这里创建套接字` |
| `miyu-ipc/src/error.rs` | `{} 上有其他内容：不是套接字的文件，或者其他程序正在监听。没有修改它` | `{} 已被占用，可能是一个不是套接字的文件，或者其他程序正在监听。没有修改它` |
| `miyu-ipc/src/error.rs` | `命名管道 {} 另一头不是当前用户的进程：不连接` | `命名管道 {} 的另一端不是当前用户的进程，因此没有连接` |
| `miyu-ipc/src/error.rs` | `{} 不属于当前用户，或者其他用户也能访问：不连接` | `{} 不属于当前用户，或者其他用户也能访问，因此没有连接` |
| `miyu-ipc/src/error.rs` | `核心没有在运行` | `核心未运行` |
| `miyu-ipc/src/error.rs` | `核心没有启动，也没有给出原因：查看数据根的 {}` | `核心没有启动，也没有给出原因，可以查看数据根中的 {}` |
| `miyu-ipc/src/files.rs` | `run/socket 写的不是绝对路径：{}` | `run/socket 中的路径不是绝对路径：{}` |

#### 命令行文案 `crates/miyu-cli/src`

共 80 处。

| 文件 | 原来 | 现在 |
|---|---|---|
| `miyu-cli/src/language.rs` | `没做` | `未执行` |
| `miyu-cli/src/language.rs` | `打断了` | `已中断` |
| `miyu-cli/src/language.rs` | `用 -o <文件> 存成文件` | `用 -o <文件> 保存到文件` |
| `miyu-cli/src/language/bus.rs` | `接进` | `接入` |
| `miyu-cli/src/language/config.rs` | `← 生效，只管这一次启动` | `← 生效，仅限本次启动` |
| `miyu-cli/src/language/config.rs` | `还没信任` | `未信任` |
| `miyu-cli/src/language/config.rs` | `比下面几层宽` | `比下面几层更宽松` |
| `miyu-cli/src/language/config.rs` | `不能写在这一层` | `不能在这一层设置` |
| `miyu-cli/src/language/config.rs` | `← 不算：{why}` | `← 未生效：{why}` |
| `miyu-cli/src/language/config.rs` | `文件尚不存在` | `文件还不存在` |
| `miyu-cli/src/language/config.rs` | `项目配置 {file} 尚未信任，本次没有使用` | `项目配置 {file} 未信任，本次没有使用` |
| `miyu-cli/src/language/confirm.rs` | `（替换已装的）` | `（替换已安装的版本）` |
| `miyu-cli/src/language/confirm.rs` | `一起装：` | `一并安装：` |
| `miyu-cli/src/language/confirm.rs` | `（社区，未经审核）` | `（社区，未经官方审核）` |
| `miyu-cli/src/language/confirm.rs` | `会删掉 {path}（{size}）` | `将删除 {path}（{size}）` |
| `miyu-cli/src/language/confirm.rs` | `配置留在 {path}` | `配置保留在 {path}` |
| `miyu-cli/src/language/confirm.rs` | `配置留在 {path}，另有 {settings} 项设置` | `配置保留在 {path}，另有 {settings} 项设置` |
| `miyu-cli/src/language/confirm.rs` | `{settings} 项设置留在配置里` | `{settings} 项设置保留在配置中` |
| `miyu-cli/src/language/confirm.rs` | `会删掉 {path}` | `将删除 {path}` |
| `miyu-cli/src/language/dev.rs` | `没通过` | `未通过` |
| `miyu-cli/src/language/dev.rs` | `跳过` | `已跳过` |
| `miyu-cli/src/language/dev.rs` | `没通过 {sample}` | `未通过 {sample}` |
| `miyu-cli/src/language/embedding.rs` | `  不自动联想：{}（这个模型没有测试过门槛，搜索不受影响）` | `  不自动联想：{}（这个模型没有实测的阈值，搜索不受影响）` |
| `miyu-cli/src/language/input.rs` | `长内容存成文件，用 --file 附上` | `把长内容保存到文件，用 --file 附上` |
| `miyu-cli/src/language/kb.rs` | `没有第 {page} 页：共 {pages} 页` | `第 {page} 页不存在：共 {pages} 页` |
| `miyu-cli/src/language/login.rs` | `以小写字母开头，只含小写字母、数字、- 和 _，最多 64 个字符` | `名字以小写字母开头，只含小写字母、数字、- 和 _，最多 64 个字符` |
| `miyu-cli/src/language/memory.rs` | `没有找到结果` | `未找到结果` |
| `miyu-cli/src/language/memory.rs` | `无需整理，用时 {took}` | `不需要整理，用时 {took}` |
| `miyu-cli/src/language/memory.rs` | `没有要改的` | `不需要修改` |
| `miyu-cli/src/language/pkg.rs` | `提示：{id} 由系统的包管理器安装，文件仍然保留，Miyu 不再加载它；要删除文件，使用系统的包管理器` | `提示：{id} 由系统的包管理器安装，文件仍然保留，Miyu 不再加载它；如果要删除文件，使用系统的包管理器` |
| `miyu-cli/src/language/pkg.rs` | `{kind} {name} 已在 {holder} 中` | `{kind} {name} 已存在于 {holder} 中` |
| `miyu-cli/src/language/pkg.rs` | `{path} 在 {id} 的目录里，不是安装时带的` | `{path} 在 {id} 的目录中，不是随软件包安装的文件` |
| `miyu-cli/src/language/pkg.rs` | `{id} 没有在运行，启用它以后再打开` | `{id} 未运行，启用它以后再打开` |
| `miyu-cli/src/language/pkg/approve.rs` | `  新要：` | `  新增：` |
| `miyu-cli/src/language/pkg/approve.rs` | `升级 '{id}' 并批准新要的能力？[Y/n] ` | `升级 '{id}' 并批准新增的能力？[Y/n] ` |
| `miyu-cli/src/language/pkg/approve.rs` | `安装 '{id}' 并批准它要的能力？[Y/n] ` | `安装 '{id}' 并批准它需要的能力？[Y/n] ` |
| `miyu-cli/src/language/pkg/approve.rs` | `升级 {count} 个软件包并批准新要的能力？[Y/n] ` | `升级 {count} 个软件包并批准新增的能力？[Y/n] ` |
| `miyu-cli/src/language/pkg/approve.rs` | `安装 {count} 个软件包并批准它们要的能力？[Y/n] ` | `安装 {count} 个软件包并批准它们需要的能力？[Y/n] ` |
| `miyu-cli/src/language/pkg/approve.rs` | `同时安装推荐的 1 个软件包并批准它要的能力？[Y/n] ` | `同时安装推荐的 1 个软件包并批准它需要的能力？[Y/n] ` |
| `miyu-cli/src/language/pkg/approve.rs` | `同时安装推荐的 {count} 个软件包并批准它们要的能力？[Y/n] ` | `同时安装推荐的 {count} 个软件包并批准它们需要的能力？[Y/n] ` |
| `miyu-cli/src/language/pkg/info.rs` | `推荐的包要的能力` | `推荐的软件包需要的能力` |
| `miyu-cli/src/language/pkg/info.rs` | `社区，没有经过官方审核` | `社区，未经官方审核` |
| `miyu-cli/src/language/pkg/info.rs` | `系统的包管理器安装` | `由系统的包管理器安装` |
| `miyu-cli/src/language/pkg/info.rs` | `{flag} 只接软件包编号` | `{flag} 只接受软件包编号` |
| `miyu-cli/src/language/pkg/install.rs` | `{source}，签名已验证，未经审核` | `{source}，签名已验证，未经官方审核` |
| `miyu-cli/src/language/pkg/pack.rs` | `无法打包：找不到图标 {logo}` | `无法打包：未找到图标 {logo}` |
| `miyu-cli/src/language/pkg/pack.rs` | `在 package.toml 的 [package] 里写上 version` | `在 package.toml 的 [package] 里填写 version` |
| `miyu-cli/src/language/pkg/source.rs` | `已更新 {source}：{count} 个包（{generated}）` | `已更新 {source}：{count} 个软件包（{generated}）` |
| `miyu-cli/src/language/pkg/source.rs` | `跳过设为不升级的 {id}` | `已跳过设为不升级的 {id}` |
| `miyu-cli/src/language/pkg/source.rs` | `{id} {to} 需要 Miyu {miyu}，跳过` | `{id} {to} 需要 Miyu {miyu}，已跳过` |
| `miyu-cli/src/language/rescue.rs` | `安装：Arch Linux 用 AUR（paru -S litemiyu 或 yay -S litemiyu）；其他发行版从 https://github.com/miyu-agent/litemiyu/releases 下载` | `在 Arch Linux 上从 AUR 安装（paru -S litemiyu 或 yay -S litemiyu）；其他发行版从 https://github.com/miyu-agent/litemiyu/releases 下载` |
| `miyu-cli/src/language/rescue.rs` | `暂只支持 Linux` | `目前只支持 Linux` |
| `miyu-cli/src/language/sandbox.rs` | `需要 Linux 5.13 或更新，并在启动参数 lsm= 里启用 Landlock` | `需要 Linux 5.13 或更高版本，并在启动参数 lsm= 里启用 Landlock` |
| `miyu-cli/src/language/sandbox.rs` | `此系统上没有可用的沙盒` | `这个系统上没有可用的沙盒` |
| `miyu-cli/src/language/schedule.rs` | `时间格式不对` | `时间格式不正确` |
| `miyu-cli/src/language/schedule.rs` | `循环，节奏自定` | `循环，由 AI 决定节奏` |
| `miyu-cli/src/language/schedule.rs` | `停着` | `已暂停` |
| `miyu-cli/src/language/schedule.rs` | `跳过：软件包未运行` | `已跳过：软件包未运行` |
| `miyu-cli/src/language/schedule.rs` | `跳过：会话已不存在` | `已跳过：会话已不存在` |
| `miyu-cli/src/language/schedule.rs` | `跳过：预设已关闭` | `已跳过：预设已关闭` |
| `miyu-cli/src/language/schedule.rs` | `跳过：无法新建会话` | `已跳过：无法新建会话` |
| `miyu-cli/src/language/setup.rs` | `无效的编号：{typed}` | `编号无效：{typed}` |
| `miyu-cli/src/language/setup.rs` | `列模型` | `列出模型` |
| `miyu-cli/src/language/setup.rs` | `发请求` | `发送请求` |
| `miyu-cli/src/language/setup.rs` | `选主对话的模型：` | `选择主对话的模型：` |
| `miyu-cli/src/language/setup.rs` | `选一个编号，直接回车用推荐的：` | `选择编号，直接回车使用推荐的模型：` |
| `miyu-cli/src/language/setup.rs` | `以小写字母开头，只含小写字母、数字、- 和 _，最多 32 个字符` | `ID 以小写字母开头，只含小写字母、数字、- 和 _，最多 32 个字符` |
| `miyu-cli/src/language/upgrade.rs` | `提示：旧版目录 {} 还在用，下次运行 'miyu upgrade' 时删除` | `提示：旧版目录 {} 仍在使用，下次运行 'miyu upgrade' 时删除` |
| `miyu-cli/src/language/upgrade.rs` | `这份 Miyu 由 {manager} 安装，用包管理器升级` | `当前的 Miyu 由 {manager} 安装，使用包管理器升级` |
| `miyu-cli/src/language/upgrade.rs` | `这份 Miyu 不是用安装脚本或包管理器装的，不自动升级` | `当前的 Miyu 不是用安装脚本或包管理器安装的，不自动升级` |
| `miyu-cli/src/language/upgrade.rs` | `提示：新版在 {page}` | `提示：从 {page} 获取新版` |
| `miyu-cli/src/language/upgrade.rs` | `{at} 以后再试` | `{at} 以后重试` |
| `miyu-cli/src/language/upgrade.rs` | `过一会儿再试` | `稍后重试` |
| `miyu-cli/src/language/upgrade.rs` | `GitHub 回应了 HTTP {status}` | `GitHub 返回 HTTP {status}` |
| `miyu-cli/src/language/upgrade.rs` | `没有找到可用的发行版` | `未找到可用的发行版` |
| `miyu-cli/src/language/upgrade.rs` | `加 '--prerelease' 可以看预发布` | `加 '--prerelease' 可以升级到预发布版` |
| `miyu-cli/src/language/upgrade.rs` | `校验和清单格式不对，不升级：{}` | `校验和清单格式不正确，不升级：{}` |
| `miyu-cli/src/core_status.rs` | `核心没有在运行` | `核心未运行` |
| `miyu-cli/src/language/kb.rs` | `已移到回收站：{trash}` | `已移入回收站：{trash}` |
| `miyu-cli/src/language/kb.rs` | `已删除 {kb}，移到回收站：{trash}` | `已删除 {kb}，已移入回收站：{trash}` |
