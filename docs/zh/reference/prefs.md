# 头自己的设置：`prefs.*`

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

这一页讲界面包（清单里有 `[ui]` 的那种软件包，也就是终端界面、网页界面这类头；头是连接核心的界面程序）自己的设置，例如皮肤、配色、主题、吉祥物。这些设置保存在核心的数据根里，跟着人走，而不是跟着地址、设备走：无论用 `127.0.0.1`、`localhost`、局域网地址还是手机打开同一个网页，看到的都是同一份设置。核心不解析里面的键，键和值都由头自己决定；只和这台设备有关的设置（栏宽、侧边栏有没有收起、输入历史）留在头自己那里。

| 方法 | 做什么 |
|---|---|
| [`prefs.get`](#prefsget) | 读取整份设置 |
| [`prefs.set`](#prefsset) | 修改几个键，返回修改后的整份设置 |

**谁能调用**：只有人可以调用（通过头的连接）；扩展、平台桥调用时返回 `local_only`。如果头要跟上其他头做的修改，可以订阅 `prefs` 流（带上 `package`），之后核心会推送 `prefs.changed`（见 [推送](pushes.md)）。

**存在哪**：设置保存在那个软件包的状态目录里的 `prefs.json`（`var/lib/<包编号>/prefs.json`，也就是 `MIYU_STATE_DIR` 指向的目录）。整份设置是一个 JSON 对象，写在一行里，在 Unix 上只有你自己能读取。这个文件由核心读写，头自己不要在状态目录里用这个名字：头往里写的内容会被当成设置读出来，格式不对的会在下一次 `prefs.set` 时被覆盖；卸载这个软件包时，设置会跟着状态目录一起删除。

**上限**：整份设置写成 JSON（不带空白）以后最多 64 KiB（65536 字节）。

## `prefs.get`

**参数**：`{"package"}`，即界面包的编号。**回应**：`{"values": {…}}`，是整份设置；还没有设置过时返回 `{}`。

## `prefs.set`

**参数**：`{"package", "values": {键: 值或 null}}`。核心按顶层的键合并：写了的键整个换成新的值（值可以是任何 JSON，对象也是整个替换，不会合并到旧对象里），写成 `null` 的键被删除，没有写的键保持不变。`values` 是 `{}` 时什么都不修改。

**回应**：`{"values": {…}}`，是合并后的整份设置。修改会推送给其他订阅了这个软件包的连接，但不推送给你自己这个连接，因为回应里已经是整份设置了。

## 订阅

发送 `subscribe {"stream": "prefs", "package"}`，回应是 `{"values": {…}}`，即这一刻的整份设置；之后其他连接修改了设置，核心会推送 `prefs.changed {"package", "values"}`，`values` 是修改后的整份设置，头用它整个替换掉自己手上的那一份。一个连接对一个软件包只有一个订阅，再订阅一次会换成新的订阅；发送 `unsubscribe {"stream": "prefs", "package"}` 可以停止订阅。如果读取得太慢、掉了队，核心会推送 `resync {"stream": "prefs", "package"}`，此时需要重新订阅。

## 出错

| 原因码 | 什么时候 |
|---|---|
| `bad_params` | 没有写 `package`；`values` 没有写，或者不是对象；订阅时没有写 `package`。 |
| `unknown_package` | 没有安装这个软件包。 |
| `not_a_ui_package` | 软件包已经安装，但不是界面包。 |
| `prefs_too_big` | 合并后的整份设置超过 64 KiB，此时什么都不修改；`data` 带 `size`（合并后有多少字节）和 `limit`（65536）。 |
| `local_only` | 调用的是扩展或者平台桥。 |

```json
→ {"jsonrpc":"2.0","id":"1","method":"prefs.set","params":{"package":"webui","values":{"skin":"paper","mascot":null}}}
← {"jsonrpc":"2.0","id":"1","result":{"values":{"scheme":"dark","skin":"paper"}}}
```
