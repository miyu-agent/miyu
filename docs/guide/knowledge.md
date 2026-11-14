# 知识库

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

知识库是你交给 AI 查的一批文档：笔记、手册、设定集、PDF。这一页讲怎么建、AI 什么时候去查、怎么更新和删。

知识库是你给的资料，原样放着，AI 查的时候读原文。[记忆](memory.md) 是 AI 在对话里自己记下的一条条的话。

AI 查知识库的工具由出厂软件包 `base-knowledge`（界面上叫「知识库」）提供。知识库的文件是你的：这个包没装，`miyu kb` 照样能管。

## 建一个

1. 建一个空库，或者把现成的目录挂进来：

   ```bash
   miyu kb new notes --name "学习笔记" --description "日语语法和单词"
   miyu kb mount handbook ~/Documents/handbook --name "团队手册"
   ```

   ```powershell
   miyu kb new notes --name "学习笔记" --description "日语语法和单词"
   miyu kb mount handbook $HOME\Documents\handbook --name "团队手册"
   ```

2. 往建的库里加文件：

   ```bash
   miyu kb add notes ~/Downloads/grammar.md ~/Downloads/kanji.pdf
   ```

   ```powershell
   miyu kb add notes $HOME\Downloads\grammar.md $HOME\Downloads\kanji.pdf
   ```

3. 试搜：

   ```sh
   miyu kb search て形
   ```

库的编号小写字母开头，只有小写字母、数字、`-`、`_`，最长 64 个字符。写错的报「知识库编号无效」。

## 能收哪些文件

| 文件 | 扩展名 | 怎么切成段 | 最大 |
|---|---|---|---|
| Markdown | `.md`、`.markdown` | 照标题 | 4 MiB |
| 纯文本 | `.txt` | 照段落 | 4 MiB |
| PDF | `.pdf` | 照页 | 64 MiB |
| 网页 | `.html`、`.htm` | 换成文字，照标题 | 64 MiB |

- 文字的要是 UTF-8。
- PDF、网页要靠和 `miyu` 放在同一个目录的 `miyu-extract`（Windows 上是 `miyu.exe` 旁边的 `miyu-extract.exe`）抽字，没有的跳过。扫描的、只有图的 PDF 没有字，也跳过。
- 子目录里的照样算。点开头的文件、目录，和链接不算。别的扩展名不算。
- `miyu kb add` 一个文件最多 20 MiB。

收不进的文件不挡别的，`miyu kb list <库>` 里那一篇后面写着为什么，例如「（无法读取：文件过大）」。

## 加进来的和挂进来的

| | `miyu kb new` + `add` | `miyu kb mount` |
|---|---|---|
| 文件在哪 | 复制进 `~/.miyu/root/kb/<编号>/`（数据根默认 `~/.miyu`，Windows 上是 `%USERPROFILE%\.miyu`） | 留在原来的目录，库里只记着挂哪 |
| 原文件改了 | 不跟着变，要再 `add --replace` | 下一次搜、列的时候跟着变 |
| 加、删文档 | 能，`add`、`remove`，AI 也能写 | 不能，只读，你和 AI 都不能 |
| `delete` | 整个库目录挪进系统回收站 | 只取消挂载，目录不动 |

挂的目录写绝对路径或 `~/` 开头，Windows 上 `~\` 开头也认。

你也能直接往 `~/.miyu/root/kb/<编号>/` 里放文件、改文件；在 `~/.miyu/root/kb/` 下建一个目录，目录名照编号的写法，就是建了一个库。下一次搜、列的时候 Miyu 照文件补索引：新的、改了的重读，删了的拿掉，没变的不动。

## miyu kb

```text
miyu kb [<子命令>] [<选项>]
```

不写子命令的是 `list`。选项写在子命令前后都行。

| 子命令 | 做什么 | 成了印 |
|---|---|---|
| `list` | 列出知识库，一行一个：编号、名字、几篇，软件包带来的写「来自 <包名>」，开着的写「自动想起」，再接说明 | |
| `list <库>` | 列出库里的文档 | |
| `search <词>…` | 搜全部库，最多 10 段 | |
| `read <库> <文档>` | 读一篇的一页。一页 32 KiB，PDF 照它自己的页 | |
| `new <库>` | 建一个空库 | 已创建 … |
| `edit <库>` | 改名字、说明，开关自动想起 | 已修改 … |
| `add <库> <文件>…` | 把文件复制进库 | 已添加 <库>/<文档> |
| `remove <库> <文档>` | 删一篇，挪进系统回收站 | 已移到回收站：… |
| `mount <库> <目录>` | 把现成的目录挂成只读的库 | 已挂载 …：N 篇 |
| `delete <库>` | 删整个库 | 已删除 …，移到回收站：… |
| `reindex <库>` | 删掉索引照文件重建 | 已重建 … 的索引：N 篇 |

| 选项 | 做什么 |
|---|---|
| `--kb <库>` | `search` 只搜这一个库 |
| `--page <页>` | `read` 读第几页，不写是第 1 页 |
| `--name <名字>` | `new`、`edit`、`mount`：显示的名字，不写的照编号 |
| `--description <说明>` | `new`、`edit`、`mount`：一句话说这个库是什么 |
| `--auto` | `new`、`edit`、`mount`：开自动想起 |
| `--no-auto` | `edit`：关自动想起 |
| `--to <目录>` | `add` 放进库里的这个子目录 |
| `--replace` | `add` 换掉已有的同名文档 |
| `--format <text\|json>` | `list`、`search` 的输出格式 |

`<文档>` 写库里的相对路径，照 `list <库>` 印的写。

```bash
miyu kb read notes grammar.md --page 2
miyu kb add notes --to 语法 --replace ~/Downloads/grammar.md
miyu kb edit notes --auto
miyu kb search --kb handbook 报销
```

```powershell
miyu kb read notes grammar.md --page 2
miyu kb add notes --to 语法 --replace $HOME\Downloads\grammar.md
miyu kb edit notes --auto
miyu kb search --kb handbook 报销
```

## AI 什么时候去查

开着知识库的会话里：

- 系统提示词里列出 AI 查得到的库：编号、名字、说明。名字、说明写清楚，AI 才知道什么时候该去查。
- AI 自己用 `kb_search` 搜，用 `kb_read` 一页一页读原文。
- 开了自动想起的库，不等 AI 去查，你说的话提到了哪一段就带上。

一个库都查不到的会话，`kb_search`、`kb_read`、`kb_edit`、`kb_delete` 不给 AI，只有 `kb_save`，好让它建第一个库。

### 自动想起

在你开的每一轮，照你这一轮说的话，从开了自动想起的库里找：

- 你的话里出现了某一节标题的最后一级（例如标题是「语法 › て形」的「て形」），带这一节。
- 你的话和某一段意思像，带这一段。要语义模型，见下面「语义搜索」。

一轮最多带 3 段，一段是它在哪（库、文档、标题）加开头 120 个字，一块不超过 600 字节；要全文 AI 自己去 `kb_read`。一段上下文里带过的不再带；和记忆的联想合起来，一段上下文（压缩以后重新算）最多带 40 段。

自动想起默认关。开关：`miyu kb edit <库> --auto`、`--no-auto`，或者网页知识库页上卡片的「自动联想」。

### 给一节加别名

Markdown 一节标题下面的头一行写关键词，这一节就多了几个触发自动想起的名字：

```markdown
## 守灯人

关键词：灯塔的姑娘、Eileen、看灯的人

守灯人住在岬角的灯塔里……
```

- 头一行以 `关键词`、`keys`、`キーワード` 开头（不分大小写），后面跟 `：` 或 `:`。
- 关键词之间用 `、`、`，`、`,`、`；`、`;` 隔开。
- 标题、关键词至少 2 个字才算。
- 这一行不带给 AI。

## AI 写知识库

| 工具 | 做什么 |
|---|---|
| `kb_save` | 新建一篇，或者接在一篇后面。库还没有的就建上 |
| `kb_edit` | 把一篇里的一段字换掉，原文要只出现一次 |
| `kb_delete` | 把一篇挪进回收站 |

- 只写得了 Markdown、纯文本，写完不超过 4 MiB。
- 挂进来的、软件包带来的库写不了。
- 删不了整个库。
- AI 用读写文件的工具碰不到 `~/.miyu/root/kb/` 里的文档，会被指去用知识库的工具。

不让 AI 写，任选一种：

- 预设的 `[tools]` 里关掉这三件：

  ```toml
  [tools]
  kb_save = false
  kb_edit = false
  kb_delete = false
  ```

- 只读的会话里这三件不执行，见 [权限和沙盒](permissions.md)。

## 哪些会话查得到哪个库

| 管的 | 怎么写 |
|---|---|
| 全部会话 | 默认查得到全部的库：你建的、挂的、装的软件包带来的 |
| 用某个人格的会话 | 人格文件 `persona.toml` 的 `[knowledge]` 写 `bases = ["notes"]`，只查得到列着的；写 `[]` 一个都查不到，也建不了库 |
| 用某个预设的会话 | `[features]` 写 `knowledge = false`，五件工具都没有，也不自动想起 |

```toml
# ~/.miyu/root/personas/captain/persona.toml
[knowledge]
bases = ["sea-charts", "handbook"]
```

建了库、改了人格，下一次就照新的。人格、预设的写法见 [人格和预设](personas-presets.md)。

## 语义搜索

搜知识库照关键词和意思两路找，合在一起排。照意思找要语义模型：`models.embedding`，默认用本机的 `embed` 软件包（`miyu pkg install embed`，见 [附加软件](addons.md)、[模型](models.md#三个用途)）。

- 没装、写了 `off` 的：只按关键词找；自动想起只认标题和关键词。
- 写了远程模型的：搜照两路找，算向量的字发给那一家；自动想起不照意思带，只认标题和关键词。

索引什么时候建：

- 关键词的索引：搜、列、挂目录、重建、AI 用知识库工具时，照文件补这个库。
- 向量：搜过以后在后台补；开了自动想起的库，用到时也在后台补。还没补上的段只有关键词找得到。

`miyu check` 末尾印索引进度，「知识库：handbook 30/42」是这个库 42 段里 30 段有了向量。怎么读见 [出了问题怎么查](troubleshooting.md#先跑-miyu-check)。

## 软件包带的知识库

软件包也能带知识库，装上就查得到，列表里写「来自 <包名>」。只读：你只能开关它的自动想起（记在 `~/.miyu/etc/kb.toml`）、重建索引。卸包时一起没了。你自己有同编号的库，用你的。

软件源里现成的有 `kb-shorinwiki`：

```sh
miyu pkg install kb-shorinwiki
```

怎么做一个见 [放文件就能用的](../authors/scripts-skills.md#知识库)。

## 界面

- 网页：设置里的「知识库」页。顶上搜；一个库一张卡片，卡片上开关「自动联想」；点进去列文档、看原文；自己建的库能「加文件」「加文件夹」、拖进来、删文档；「新建空库」「挂一个现有文件夹」「重建索引」「删除」。
- 终端没有知识库页，用 `miyu kb`。

## 存在哪

| 位置 | 是什么 |
|---|---|
| `~/.miyu/root/kb/<编号>/` | 一个库：文档，和一份 `kb.toml` |
| `~/.miyu/var/cache/index/kb/` | 索引，照文件算出来的，`reindex` 重建 |
| `~/.miyu/etc/kb.toml` | 软件包带来的库，你改过的自动想起开关 |

`kb.toml` 能手写，四格都可以不写：

```toml
name = "学习笔记"             # 名字，不写的照编号
description = "日语语法和单词"  # 一句话说明
auto = true                  # 自动想起，不写是关
mount = "~/Documents/notes"  # 挂进来的目录；写了的，文档在那里
```

Windows 上 `mount` 写 Windows 的路径，用 TOML 的单引号字符串，里面的 `\` 照原样：`mount = 'D:\notes'`。

写了别的格、写法不对的，这个库照没有 `kb.toml` 用，`miyu kb list` 里带着错。

备份、搬家见 [数据和备份](data.md)。

## 出了问题

| 看到的 | 怎么办 |
|---|---|
| 「无法添加 X：不支持的格式」 | 只收上面表里的几种 |
| 「挂进来的知识库是只读的。」 | 挂的库不能加、删，到原目录里改 |
| 「随软件包来的知识库是只读的，只能开关自动想起。」 | 软件包带来的库只能开关自动想起、重建索引 |
| 「这个目录挂不上。」 | 目录不在，或者读不了 |
| 文档后面写「缺少 miyu-extract」 | PDF、网页抽不了字，`miyu-extract` 要和 `miyu` 在同一个目录 |
| AI 不去查 | `miyu kb search` 试搜；看人格的 `[knowledge]`、预设开没开知识库；给库写清楚名字、说明 |
