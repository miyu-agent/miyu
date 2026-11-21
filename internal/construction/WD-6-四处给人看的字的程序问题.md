## 施工单 WD-6：四处给人看的字的程序问题

状态：做完了，等合并（分支 `step/WD-6-四处给人看的字的程序问题`，从 main 的 62d294acd 开）。主会话 2026-11-21 派：WD-5 真跑和读代码时报上去的四处，每一处先证明修之前出现，再修。不改协议字段，各个界面不用跟着改。

### 四处问题和修法

#### 一、`miyu pkg pack` 在中文界面输出英文的原因

- **修之前**：清单写错时，命令行把清单问题的英文原话接在「无法打包：」后面。

  ```
  miyu: 无法打包：line 2: package.id is not a known key
  ```

  同一个目录用 `miyu pkg info` 看，核心说的是中文「package.toml:2: 未知的键：package.id」。
- **原因**：`crates/miyu-cli/src/pkg/pack.rs` 不连核心，直接用了 `miyu_config::package::Problem` 的 `Display`（英文，给协议和运行日志用）。
- **修法**：打包失败时从资源目录读一份字表，按问题的代码取核心装包时说的同一句（`package-problems/<代码>`），知道第几行的带上 `package.toml:<行>`。读不到字表时使用英文原话。英文界面也改成同一个格式。

  ```
  miyu: 无法打包：package.toml:2: 未知的键：package.id
  miyu: cannot pack: package.toml:2: Unknown key: package.id
  ```

#### 二、`miyu kb mount` 失败时中文句子后面接英文的系统报错

- **修之前**：

  ```
  miyu: 无法挂载这个目录：mounted folder /…/nowhere: No such file or directory (os error 2)
  ```

  `miyu kb list` 里挂着的目录用不了时也一样：`（不可用：mounted folder /mnt/usb: No such file or directory (os error 2)）`。
- **原因**：核心在拒绝的 `data.message` 里给一句英文（`crates/miyu-store/src/knowledge/mount.rs` 的 `mounted`），命令行原样接在中文后面。
- **修法**：`data.message` 不变。命令行在中文界面把这一句换成中文（`Language::folder_why`）：按系统的错误号认出是哪一种，认得出的说自己的话，认不出的带上系统的原话。英文界面保持原样。

  | 情形 | 中文界面输出 |
  |---|---|
  | 目录不存在 | 无法挂载这个目录：目录 <路径> 不存在 |
  | 给的是文件 | 无法挂载这个目录：<路径> 不是目录 |
  | 没有权限 | 无法挂载这个目录：没有权限读取目录 <路径> |
  | 其他 | 无法挂载这个目录：无法读取目录 <路径>，系统报错：<原话> |
  | 不是绝对路径，也不以 `~/` 开头 | 无法挂载这个目录：<写的> 不是绝对路径，也不是以 ~/ 开头的路径 |
  | 挂上以后目录没了（`kb list`） | docs  1 篇（不可用：目录 <路径> 不存在） |

#### 三、长期目标失败时，人看到的是英文原因码

- **修之前**：`goal` 工具被内核拒绝时，给人看的那一句一律是「目标未更改：{reason}」，`{reason}` 填的是原因码，人看到的是「目标未更改：goal_paused」「目标未更改：stopped」。
- **原因**：`crates/miyu-goal/src/goal.rs` 的 `refused` 把给模型的那一句和给人看的那一句用了同一组字段。
- **修法**：给模型的那一句不变（`The goal was not changed: goal_paused.`）。给人看的那一句按情形各说一句，不带原因码：

  | 情形 | 说法的键 | 中文 | 英文 |
  |---|---|---|---|
  | 目标已暂停 | `goal/paused` | 目标已暂停，未更改 | Goal is paused; not changed |
  | 目标正在进行 | `goal/running` | 目标正在进行，未更改 | Goal is running; not changed |
  | 这次调用被叫停、会话停了 | `goal/stopped` | 调用已中断，目标未更改 | Call interrupted; goal not changed |
  | 其他原因 | `goal/unchanged` | 目标未更改 | Goal not changed |

  旧的 `goal/failed` 留在字表里，以前的会话记录仍然读得出。新加的四句三种语言都有。

#### 四、循环没有下一次时间时显示「下次 」

- **修之前**：`schedule_next` 工具收到的答复里没有下一次的时刻时，给人看的是「下次 」，后面是空的；给模型的是 `Next iteration at .`。
- **实际会不会出现**：读了 `crates/miyu-endpoint/src/schedules/port.rs`，真的端口总是先定好时刻再答，所以正常使用时不会出现。它只在端口答错时出现，用测试里的假端口可以复现。
- **修法**：没有时刻的答复按端口答错处理（`crates/miyu-schedule/src/next.rs`），和这个函数里其他答错的情形一样，说「这个会话不支持循环」，不再输出半句话。没有新加句子，给模型的字没有改动。

### 先红后绿

四个测试先写，在没有改程序的树上运行，全部失败（另一台测试机，`cargo test`）：

| 测试 | 修之前的失败 |
|---|---|
| `miyu` 的 `pkg_pack::a_broken_manifest_is_said_in_the_interface_language` | 左边是 `miyu: 无法打包：line 2: package.id is not a known key` |
| `miyu-cli` 的 `kb::mounting_a_folder_and_deleting_it_from_the_shell` | 输出是 `miyu: 无法挂载这个目录：mounted folder /tmp/…/nowhere: No such file or directory (os error 2)` |
| `miyu-goal` 的 `tool::refusals_say_what_went_wrong` | 说法的键是 `software/goal/goal/failed`，期望 `software/goal/goal/paused` |
| `miyu-schedule` 的 `next::a_wakeup_without_a_time_is_not_said_as_set` | 没有时刻的答复没有算作出错 |

改完程序以后四个都通过。另外加了 `miyu-cli` 的单元测试 `why_a_folder_is_unavailable_is_said_in_chinese`（五种原话各换成什么、英文保持原样、没有权限的情形）。

### 真跑的结果

在另一台测试机上用沙箱数据根（`HOME`、`MIYU_HOME` 都指向临时目录）实际运行：

```
$ miyu pkg pack xpkg                         （清单第 2 行写了不存在的键）
miyu: 无法打包：package.toml:2: 未知的键：package.id

$ miyu pkg pack xpkg                         （name 写成了数字）
miyu: 无法打包：package.toml:3: package.name 必须是从语言到文本的表，语言只能是 zh、en、ja

$ miyu pkg pack xpkg                         （TOML 没有写完）
miyu: 无法打包：package.toml:1: 无法读取 TOML：unclosed table, expected `]`

$ LANG=en_US.UTF-8 miyu pkg pack xpkg
miyu: cannot pack: package.toml:2: Unknown key: package.id

$ MIYU_RESOURCES=<不存在的目录> miyu pkg pack xpkg     （读不到字表）
miyu: 无法打包：package.toml:2: package.id is not a known key

$ miyu kb mount gone <沙箱>/work/nowhere
miyu: 无法挂载这个目录：目录 <沙箱>/work/nowhere 不存在

$ miyu kb mount afile <沙箱>/work/afile      （是文件）
miyu: 无法挂载这个目录：<沙箱>/work/afile 不是目录

$ miyu kb mount locked <沙箱>/work/locked    （chmod 000）
miyu: 无法挂载这个目录：没有权限读取目录 <沙箱>/work/locked

$ LANG=en_US.UTF-8 miyu kb mount gone <沙箱>/work/nowhere
miyu: folder cannot be mounted: mounted folder <沙箱>/work/nowhere: No such file or directory (os error 2)

$ miyu kb list                               （挂上以后把目录挪走）
docs  1 篇（不可用：目录 <沙箱>/work/docs 不存在）
```

长期目标和循环两处要模型调用工具才会走到，没有用真模型跑，由上面的测试覆盖。

### 没有改动的

- 协议字段没有改动：拒绝的 `data.message`、`knowledge.list` 的 `error` 仍然是核心给的英文一句，各个界面不用跟着改。
- 给模型的字没有改动：`goal`、`schedule_next` 给模型的那几句和原来逐字相同。
- 第三句 TOML 的例子里，冒号后面的 `unclosed table, expected …` 是 TOML 解析器的原话，安装时也是这样显示，这一步没有处理。

### 连带的修改

- 手册 `docs/zh/dev/packages.md`、`docs/en/dev/packages.md` 的「常见报错」表各加一行（`无法打包：package.toml:<行>: …`）。
- 蓝图 `cli/kb.md` 的列表例子和「用不了」那一行、`tools/goal.md` 的句子表跟着更新。

### 验收

- 上面四个测试先红后绿；全仓 `cargo test --workspace`、门禁十二项通过；三个平台的 CI 通过。
- `python3 ~/.cache/miyu-docs-2026-11-14/lang/check.py .`：中英各 62 页，问题 0 个。
