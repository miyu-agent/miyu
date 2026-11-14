# 往界面上放东西

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

你写了一个扩展，想让它在终端、网页上露面：侧边栏里一块状态、首页一段、底栏一小格、一整页可以改的设置、一个自己的后台网页、托盘菜单里一行。这一页讲每种怎么做。扩展本身怎么写见 [写扩展](../authors/extensions.md)。

## 有哪几种

| 办法 | 出现在哪 | 清单 | 要的能力 |
|---|---|---|---|
| 界面块：首页 | 终端：空会话首页工作目录下面；网页：新会话的首页 | `[[widget]]`，`place = "home"` | `widgets` |
| 界面块：侧边栏 | 终端：右边的侧边栏（窗口 120 列以上才有）；网页：右边一栏的「面板」 | `[[widget]]`，`place = "sidebar"` | `widgets` |
| 界面块：底栏 | 终端：输入框下面那一行；网页：输入框下面那一行的右边 | `[[widget]]`，`place = "status"` | `widgets` |
| 界面块：整页 | 终端：居中的一整页；网页：软件的独立整页。敲它的斜杠命令打开 | `[[widget]]`，`place = "page"` | `widgets` |
| 斜杠命令 | 两个头的命令菜单 | `[[slash]]` | `commands` |
| 后台页 | 网页设置页「软件后台」；终端「软件包」页的「软件后台」；`miyu pkg page <编号>` | `[page]` | 自己起网页服务、替页面要登录票的要 `login.page` |
| 托盘菜单一行 | 托盘菜单（macOS 上是菜单栏图标） | `[tray]`，要有 `[page]` | 不用另要 |

这几样都要包带常驻的程序：清单有 `[process]`（扩展），能力写进 `[process] capabilities`。网页的配色和形状见 [写主题和外观](themes.md)。

## 界面块

清单里声明有哪几块，内容由你的程序推给核心，核心转给正在看的头，头自己画。

### 清单

```toml
[process]
args = ["serve"]
capabilities = ["widgets"]

[[widget]]
id = "now"
place = "sidebar"
title = { zh = "时间", en = "Time" }
```

| 格 | 说明 |
|---|---|
| `id` | 包里唯一：小写字母开头，小写字母、数字、`-`，最多 32 个 |
| `place` | `home` 首页、`sidebar` 侧边栏、`status` 底栏、`page` 整页 |
| `title` | 侧边栏、整页必写：`{ zh = "…", en = "…" }` |
| `scope` | `global`（默认）全局一份；`session` 一个会话一份，只给侧边栏 |
| `command` | 只给整页：敲这个斜杠命令打开这一页，写法同 `[command] name` |
| `state` | `true`：内容来自包在会话里的状态（工具结果的 `state`、`session.set_state`），程序没在跑也有；只给 `scope = "session"` 的块 |

一个包最多 16 块。写错的报 `bad_widget`，`miyu pkg install`、`miyu check` 会说是哪一格。

### 程序那边

1. 有人开始看一块时，核心发通知 `widget.watch {"widget", "session"?}`；最后一个人也不看了，发 `widget.unwatch`。你的程序连上握完手，核心把这一刻有人在看的各补发一条 `widget.watch`。只在有人看的时候取数据。
2. 推内容：`widget.set {"widget", "session"?, "title"?, "content"}`。跟着会话的块必写 `session`，全局的不能写。`title` 换掉清单里的标题。回 `{"kept": true}`；跟着会话、这个会话没人在看的回 `{"kept": false}`，没存。
3. 撤掉：`widget.clear {"widget", "session"?}`。
4. 块上能点的东西调你的方法：先 `package.methods {"methods": [{"name": "refresh"}]}` 登记，人点了核心发 `method.call {"method", "params"}`，你回什么头拿到什么。

每推一次整块换掉。内容只在核心的内存里：核心、你的程序重启以后都从空的开始，等下一次 `widget.watch` 再推。你的程序停了，它的块全清，头写「<软件名> 没有运行」（网页写「未运行」）。同一块推给同一个头最快 250 毫秒一次，中间的只留最新的。

方法的每一格、出错见 [扩展参考](../reference/extensions.md#widgetsetwidgetclearwidgetwatchwidgetunwatch)。

### 内容

一个数组，一项一段，每段用 `type` 说是哪种：

| `type` | 格 |
|---|---|
| `heading` | `text` |
| `text` | `text`（能换行，最多 2000 个字）、`tone` |
| `pairs` | `items`：`{"label", "value", "tone"?}`，至多 50 对 |
| `progress` | `value`（0 到 1，必写）、`text`、`mark`（0 到 1，条上标的那一处） |
| `list` | `items`：`{"title", "detail"?, "right"?, "state"?, "action"?}`，至多 200 条；`right` 是一段字，或者至多 4 段 `{"text", "tone"?}` |
| `buttons` | `items`：`{"label", "action"}`，至多 8 个 |
| `sections` | 只给整页：`items`（`{"id", "title", "subtitle"?, "badge"?, "state"?, "blocks"}`）、`add`（`{"label", "call", "options"?}`） |
| `form` | 只给整页：`id`、`fields`（`{"key", "label", "hint"?, "control", "value", …}`）、`submit`（`{"label", "call", "params"?}`） |

```json
[
  {"type": "heading", "text": "本周"},
  {"type": "text", "text": {"zh": "晴 23°C", "en": "Sunny 23°C"}, "tone": "accent"},
  {"type": "pairs", "items": [{"label": "湿度", "value": "40%"}]},
  {"type": "progress", "value": 0.42, "text": "索引 42%"},
  {"type": "list", "items": [{"title": "构建 #128", "right": "3m", "state": "doing", "action": {"call": "build.open", "params": {"id": 128}}}]},
  {"type": "buttons", "items": [{"label": "刷新", "action": {"call": "refresh"}}, {"label": "详情", "action": {"open": "detail"}}]}
]
```

- 字是字符串，或者语言到字的表（`{"zh": "…", "en": "…"}`，认 `zh`、`en`、`ja`），核心照头的语言挑。`text` 段的字最多 2000 个，别的最多 200 个。只画成字：不认 Markdown、HTML，不许有控制字符（`text` 段的换行除外）。
- `tone`：`normal`、`dim`、`accent`、`good`、`warn`、`bad`，颜色由头照自己的配色定。列表的 `state`：`todo`、`doing`、`done`、`failed`、`off`；分区的 `state`：`good`、`warn`、`bad`、`off`。
- `action`：`{"call": <你登记的方法>, "params"?}`；`{"open": <你的一个整页块的 id>}`；`{"copy": <字>}` 把字复制到剪贴板。
- 整块最多 64 段、写成 JSON 最多 64 KiB；`home` 最多 8 段；`status` 只能是一段一行的 `text`，最多 40 列（中文算两列）。

**整页的表单**：`form` 的 `control` 是 `toggle`、`select`（带 `options: [{"value", "label"}]`）、`text`、`textarea`、`list`、`number`（可带 `min`、`max`、`integer`）。`value` 写 `null` 表示照默认，`default` 写 `{"text", "value"?}` 告诉头默认是什么。人点保存，头经 `package.call` 调你的 `submit.call`，参数是 `submit.params` 加上 `{"values": {只有改过的格}}`。你回 `{"saved": true}`；`{"saved": false, "problems": [{"key", "message"}]}`（写在那一格下面）；或 `{"saved": false, "message"}`（写在表单底下）。存好以后照常 `widget.set` 推新的内容。`sections` 的 `add` 被点了，调 `add.call`，参数 `{"value": <选的那一项>}`，可以回 `{"id": <新的那一项>}`。

### 两个头怎么画

| | 终端 | 网页 |
|---|---|---|
| 首页 | 至多 3 块，一块至多 6 行，合起来至多 12 行 | 至多 3 块，一块至多 8 行 |
| 侧边栏 | 一块至多 10 行；排在侧边栏自带的几段后面 | 「面板」里至多 8 块，一块至多 12 行 |
| 底栏 | 至多 3 格，一格至多 40 列 | 至多 3 格 |
| 整页 | 居中，滚着看 | 软件的独立整页 |
| `sections`、`form` | 不画 | 画，能改、能存 |

终端里放不下的：列表照待办的样子收起，`text` 截掉加 `…`，别的整段放不下就连同后面的不画；点块里别处展开、再点收起。终端侧边栏里的先后可以在配置文件里排：`tui.side` 一项一件，你的块写成 `pkg:<包的编号>/<块的 id>`。

### 整页和斜杠命令

写了 `command` 的整页块，它的命令列进 `command.catalog`（带 `opens`）。人敲了，`command.run` 回 `{"command", "opens": {"package", "widget"}, "said": ""}`，头打开那一页；不进会话日志。通讯平台上的群、私聊里不列、不认这种命令。

只想回一句话的斜杠命令写 `[[slash]]`，人敲了核心调你的 `command.call`，你回 `{"said": "<回执>"}`，见 [写扩展](../authors/extensions.md)。

## 后台页

给你的包一个网页：设置、管理、看数据。清单二选一：

- `[page] dir = "page"`：静态文件，目录里要有 `index.html`。
- `[page] server = true`：你的程序自己起网页服务。

### 静态的后台页

打开的地方：网页设置页「软件后台」，终端「软件包」页的「软件后台」（`Enter` 在浏览器打开），命令行 `miyu pkg page <编号>`（`--print` 只印网址）。

- 不装网页也能开：`miyu pkg page` 让核心在 `127.0.0.1` 上起一个后台页服务，十分钟没有请求、没有开着的推送流就关。
- 页面放在一个隔离的框里，经一条消息通道和外面说话。框载入完，外面 `postMessage({"miyu": "port"}, "*", [port])` 交一个 `MessagePort` 进来，只交一次，所以页面要做成单页（在框里跳到别的页就没有通道了）。
- 通道上照 JSON-RPC 2.0 说，你能调的只有四样：

  | 方法 | 参数 | 回 |
  |---|---|---|
  | `context` | 无 | `{"package", "language", "theme", "colors"?}`：你的包、语言、深浅（`light`、`dark`）；网页里另带几个主色 |
  | `settings.get` | 无 | `{"items", "values"}`：你这个包的配置项和最终值 |
  | `settings.set` | `{"changes": [...]}`，写法同 `config.set` | 只能写 `<你的前缀>.` 开头的键，别的整个不办，回 `forbidden` |
  | `call` | `{"method", "params"?}` | 转给你的程序（`method.call`），你回什么页面拿到什么 |

  外面另推：`settings.changed`（你的配置项变了）、`theme.changed`（换了深浅）、`package.event`（你的程序经 `package.emit` 推给页面的）、`resync`（推送掉了队，重新拉一遍）。
- 照着抄的例子：表情包 `memes` 包的 `package/page/`（`app.js` 里的通道写法）。

### 自己起服务的后台页

- 只有扩展能写。用 `package.methods` 登记一个 `status` 方法，回应里写 `web`：服务实际听的端口（还没听上的不写）。
- 头和 `miyu pkg page` 照它开 `http://127.0.0.1:<端口>/#miyu=<…>`：`#` 后面是一段 base64url（不带 `=`）编的 JSON `{"v": 1, "ticket", "lang"}`，`ticket` 是一张 60 秒、一次性的登录票。你的服务拿它换设备令牌：在扩展的连接上调 `login.redeem {"ticket"}`，回 `{"login": {"token", "expires"}}`，之后照这个设备令牌（握手凭据 `login`）替页面连核心。
- 你自己要票（例如页面过期了重开）：声明能力 `login.page`，在扩展连接上调 `login.ticket {"via": "page"}`。这种票只在本机的连接上换得成；只给核过是本机来的请求要票、换票，远处来的页面一律走访问密码。要守的几条见 [扩展参考](../reference/extensions.md) 的「替本机页面换登录票」。
- 例子：接入通讯平台的 `im` 包（清单 `im/package.toml`）。

## 托盘菜单一行

```toml
[page]
dir = "page"

[tray]
name = { zh = "打开时钟后台", en = "Open Clock" }
```

托盘菜单里多一行，点了用浏览器打开你的后台页（同 `miyu pkg page <编号>`）。一个包一行，只能开自己的后台页。没写 `[page]`、写了 `name` 以外的格报 `bad_tray`。包要装着、开着，托盘才列它。

## 网页里的前端包

出厂网页是一个个网页包拼起来的（每个是一个 `manifest.json` 加 `index.js`）。第三方能带进网页的只有外观包的 `[theme.web]`，对外说明的用途是配色方案、主题、吉祥物（见 [写主题和外观](themes.md)）。往网页里加别的功能（新的面板、按钮、页面），现在没有对第三方开放的写法：用上面的界面块和后台页。

## 例子：侧边栏里的时钟

一个扩展，在侧边栏放一块「时间」，显示现在几点和日期，每分钟换一次。用 Python 开发包 `miyu_ext` 写（[写扩展](../authors/extensions.md#用开发包写)）。

1. 建目录，把开发包的 `miyu_ext.py` 拷进 `bin/`：

   ```text
   clock/
   ├── package.toml
   └── bin/
       ├── miyu-clock      # 下面的程序，Linux、macOS 上带执行权限
       └── miyu_ext.py
   ```

2. `package.toml`：

   ```toml
   [package]
   version = "0.1.0"
   protocol = [1, 1]
   name = { zh = "时钟", en = "Clock" }
   summary = { zh = "侧边栏显示现在几点", en = "Shows the time in the sidebar" }

   [command]
   name = "clock"
   program = "miyu-clock"
   about = { zh = "时钟扩展", en = "Clock extension" }

   [process]
   args = ["serve"]
   capabilities = ["widgets"]

   [[widget]]
   id = "now"
   place = "sidebar"
   title = { zh = "时间", en = "Time" }
   ```

3. `bin/miyu-clock`：

   ```python
   #!/usr/bin/env python3
   """侧边栏里的时钟：每分钟推一次现在几点。"""
   import os
   import sys
   import threading
   import time

   sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

   from miyu_ext import Extension  # noqa: E402

   ext = Extension("clock", "0.1.0")
   wake = threading.Event()

   @ext.on("widget.watch")
   def watched(params):
       wake.set()                     # 有人开始看了：马上推一次

   @ext.ready
   def tick():
       while True:
           now = time.localtime()
           ext.call("widget.set", {"widget": "now", "content": [
               {"type": "text", "text": time.strftime("%H:%M", now), "tone": "accent"},
               {"type": "pairs", "items": [
                   {"label": {"zh": "日期", "en": "Date"}, "value": time.strftime("%Y-%m-%d", now)},
               ]},
           ]})
           wake.wait(60 - now.tm_sec)     # 到下一分钟，或者有人开始看
           wake.clear()

   if __name__ == "__main__":
       if sys.argv[1:] == ["serve"]:
           ext.run()
       else:
           print("usage: miyu-clock serve (started by the Miyu core)", file=sys.stderr)
           sys.exit(2)
   ```

   Windows 上这份文件照样用：核心没找到 `bin/miyu-clock.exe`，就照第一行的 `#!` 找 `PATH` 里的 `python` 起它，不用执行权限。文件名照清单的 `program` 写 `miyu-clock`，不加 `.py`；要装好 Python 3，`python` 在 `PATH` 里找得到。见 [写一个 Miyu 软件包：三个平台](../authors/README.md#三个平台)。

4. 装：`miyu pkg install ./clock`。那一屏列出它要的能力「widgets」，答 `Y`；问「现在打开？」也答是。
5. 看：
   - 终端：窗口拉到 120 列以上，右边侧边栏里多一段「时间」，下面一行强调色的 `14:05`，再一行「日期 2026-11-14」。
   - 网页：点开右边一栏的「面板」，里面一块「时间」，内容同上。

没出现的：「软件包」页看它是不是在跑，再看数据根（默认 `~/.miyu`，Windows 上是 `%USERPROFILE%\.miyu`）里的 `var/log/clock/stderr`。

## 出了问题

| 看到的 | 怎么办 |
|---|---|
| `widget.set` 回 `bad_widget_content` | `data.path` 是错在哪（例如 `content[1].items[0].label`），`data.problem` 是哪一种：`type`、`missing`、`too_long`、`too_many`、`too_big`、`control_char`、`tone`、`state`、`value`、`action`、`place`、`unknown`、`duplicate` |
| `widget.set` 回 `unknown_widget` | `widget` 不是清单里写的 `id` |
| `widget.set` 回 `bad_params` | 跟着会话的块没写 `session`，或者全局的写了 |
| 调方法回 `needs_approval` | 清单的 `capabilities` 没写 `widgets`（或别的那一项），加上再装一次 |
| 块上写「没有运行」 | 程序没在跑：「软件包」页看原因和标准错误的最后几行 |
| 托盘里没有那一行 | 包没开着，或者后台页的 `index.html` 不在 |
