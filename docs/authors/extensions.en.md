# Writing an extension

An extension is a standalone program in any language. The core starts it and speaks JSON-RPC 2.0 with it over stdin and stdout, one message per line. The extension shakes hands, then registers its tools; when the AI calls one, the core asks the extension to do it. Chinese version: [extensions.md](extensions.md). Every method in full: [protocol reference: extensions](../reference/extensions.en.md).

## A minimal extension

```text
hello/
├── package.toml
└── bin/
    └── miyu-hello
```

`package.toml`:

```toml
[package]
protocol = [1, 1]
name = { en = "Hello", zh = "打招呼" }

[command]
name = "hello"
program = "miyu-hello"
about = { en = "Hello extension", zh = "打招呼扩展" }

[process]
args = ["serve"]
capabilities = ["tools"]

[settings.greeting]
type = "text"
max = 40
default = "Hello"
layers = ["system"]
applies = "now"
name = { en = "Greeting", zh = "问候语" }
```

`bin/miyu-hello` (Python; `chmod +x` on Linux and macOS):

```python
#!/usr/bin/env python3
import json
import sys

TOOLS = [{
    "name": "hello_greet",
    "description": "Greet someone by name.",
    "input_schema": {"type": "object", "properties": {"name": {"type": "string"}}, "required": ["name"]},
    "access": "read",
    "venues": ["local"],
    "display_name": {"en": "Greet", "zh": "打招呼"},
    "subject": "name",
}]


def send(message):
    sys.stdout.write(json.dumps(message, ensure_ascii=False) + "\n")
    sys.stdout.flush()


def serve():
    send({"jsonrpc": "2.0", "id": "h1", "method": "hello",
          "params": {"protocol": [1, 1], "head": {"kind": "hello", "version": "0.1.0"}}})
    greeting = "Hello"
    for line in sys.stdin:  # reaching the end means the core asks you to exit
        message = json.loads(line)
        if message.get("id") == "h1":
            if "error" in message:
                sys.exit(1)  # handshake refused: restarting won't help
            greeting = message["result"].get("config", {}).get("hello.greeting", greeting)
            send({"jsonrpc": "2.0", "id": "p1", "method": "provide", "params": {"tools": TOOLS}})
        elif message.get("method") == "extension.config":
            greeting = message["params"]["keys"].get("hello.greeting") or "Hello"
        elif message.get("method") == "tool.call":
            name = message["params"]["args"].get("name", "there")
            send({"jsonrpc": "2.0", "id": message["id"],
                  "result": {"blocks": [{"type": "text", "text": f"{greeting}, {name}!"}]}})


if __name__ == "__main__":
    if sys.argv[1:] == ["serve"]:
        serve()
    else:
        print("usage: miyu hello serve (started by the Miyu core)", file=sys.stderr)
        sys.exit(2)
```

Try it: `miyu pkg install ./hello`, answer yes to "Turn it on now?", then ask the AI to "say hello to Ann". On Windows, `bin/` needs an `.exe`: package it with PyInstaller, or use a language that compiles to an executable.

## Being started

- The program is the one in `bin/`; its arguments are `[process] args`.
- The environment is the core's, plus `MIYU_HOME` (data root), `MIYU_RESOURCES`, `MIYU_PACKAGE_DIR` (your package folder: find files you bring from here; don't build paths yourself). `MIYU_PACKAGE_DIR` is also set when a person runs `miyu <your command>` in a terminal.
- The working directory is your package's state directory `~/.miyu/state/packages/<id>/`: put your database and caches there; they're removed on uninstall.
- Stderr goes to `~/.miyu/state/logs/<id>.stderr`; look there first when something goes wrong.

## Handshake and settings

Send `hello` first, without credentials. In the response:

- `config`: the final values of your package's settings (`{"hello.greeting": "Hello"}`, secrets as real values, unset keys left out). **Don't read config files yourself.**
- `language`: which language to use for text shown to people (`zh`, `en`, `ja`).
- `account`: who you are (the system account if you declared one).

When settings change later, the core sends the notification `extension.config {"keys": {"hello.greeting": "Hi"}}` with only the changed keys; ones that became unset are `null`. A connection that hasn't shaken hands within 10 seconds is closed.

## Registering tools: `provide`

After the handshake, send `provide`; `tools` is **all** the tools you provide now, replacing the previous set each time. A tool:

| Field | Notes |
|---|---|
| `name` | Letters, digits, `_`, `-`, 1 to 64; must not clash; prefixing with the package id helps |
| `description`, `input_schema` | For the model: short English sentences, at most three per tool |
| `access` | What it touches: `read` (only looks, stays inside Miyu), `write` (changes files), `execute` (runs commands), `network`, `outbound` (sends things out of Miyu), `venue` (actions on a chat platform: recall, mute). Permissions decide whether to ask a person from this, so **be honest** |
| `venues` | Which sessions: `local` (this machine), `private` (platform private chats), `group` (groups) |
| `timeout_ms` | 1000 to 600000, default 60000 |
| `display_name` | Name for people, per language: `{"zh": "打招呼", "en": "Greet"}` |
| `subject` | Which argument's value the timeline shows after the name: "Greet · Ann". Tools that search, or act on a particular file or id, should set it: without it the timeline shows only the name, and nobody can tell what was searched or which one was touched |
| `need_load` | `true`: the AI normally sees only the name and one line, and loads it when needed |

A mistake rejects the whole set; the response's `data` says which tool and what's wrong (`{"tool": "hello_greet", "problem": "name"}`).

## Being called: `tool.call`

```json
{"jsonrpc":"2.0","id":"core-1","method":"tool.call","params":{"session":"…","call_id":"…","tool":"hello_greet","args":{"name":"Ann"},"cwd":"/home/me/src","owner":true}}
```

Response:

```json
{"jsonrpc":"2.0","id":"core-1","result":{"blocks":[{"type":"text","text":"Hello, Ann!"}]}}
```

- `args` are already corrected against your `input_schema`. `by` is who asked her; `owner` is whether the core recognizes that person as the owner. For tools that act in groups, check them yourself to decide who may use them.
- `blocks` is content for the model: text `{"type": "text", "text"}`; for images, upload with `blob.put` first, then `{"type": "image", "blob", "media_type", "width", "height"}`.
- On failure, still return a result with `error: true` and the reason written for the model; a JSON-RPC error also counts as failure.
- To have her "send" an image (stickers and the like, drawn in the reply, sent as an image on chat platforms): add `"sent": [{"blob", "media_type", "width", "height", "alt"?, "sticker"?}]` to the result, at most 4. The AI doesn't see `sent`.
- Several `tool.call`s may be in flight at once; answer each by its `id`.
- After `timeout_ms`, or when a person interrupts, the core sends the notification `tool.cancel {"session", "call_id"}`: drop that call.

## Exiting and crashing

- Stdin reaching its end means "please exit", on all three platforms; if you haven't exited after 5 seconds you're killed.
- Exiting on your own (exit code 0 included), a failed handshake, or closed stdout counts as a failure; the core restarts you after 1, 2, 4, 8, 16 seconds and stops after 5 in a row.
- **Exit code 1 is for problems a restart won't fix, like bad config or a taken port**: the core stops at once without retrying, and the person fixes it and turns you on again.
- After 60 seconds of running, the failure count starts over.

## More abilities

Each needs the matching capability declared in the manifest (see [The manifest](manifest.en.md)).

**Your own slash commands** (`commands`): declare `[[slash]]`. When a person types one, the core sends `command.call {"command", "argument", "text", "session", "by", "owner", "language"}`; answer `{"said": "<reply, may span lines>"}`, shown to the person as-is. Your program must `provide` once after starting (an empty list if you have no tools) so the core knows your connection.

**Adding a few lines to her context at the start of a turn** (`context.inject`): at the start of every turn the core sends `user.append {"session", "venue", "text"?}`; answer within 300 ms with `{"content": [{"kind", "text"}]}` (at most 3 blocks), or `{"content": []}` when there's nothing to add. Only extensions with a tool in the session's tool set are asked. This is text for the model: short English, wrapped in an XML tag.

**A section at the end of the system prompt** (`context.system`): call `system.append {"content", "session"?}`. Without `session` it applies to all sessions; `content: null` removes it. A change recomputes the whole system prompt at the next turn, so don't change it often; for a few fixed lines, call it once at startup.

**Searching by meaning**: call `model.embed {"texts": [...]}` for vectors (using the embedding model the person chose in settings). Without one configured you get `embedding_off`; fall back to keyword search. No extra capability needed.

**Calling a model once**: `model.call` (e.g. having the AI describe an image with `"use": "vision"`). It spends the person's budget, so use it sparingly.

**Methods for your page and widgets**: register with `package.methods {"methods": [{"name": "status"}]}`; when the page or a button calls one, the core sends `method.call {"method", "params"}` and your answer is what the caller gets. Answer unknown methods with JSON-RPC `-32601`.

**Widgets** (`widgets`): declare `[[widget]]`. When someone starts looking at one, the core sends `widget.watch {"widget", "session"?}`; when the last viewer leaves, `widget.unwatch`. Only fetch data while someone is looking; don't poll on a timer. Push content with `widget.set {"widget", "session"?, "title"?, "content"}`:

```json
[{"type":"heading","text":"This week"},{"type":"text","text":{"zh":"晴 23°C","en":"Sunny 23°C"},"tone":"accent"},{"type":"pairs","items":[{"label":"Humidity","value":"40%"}]},{"type":"progress","value":0.4,"text":"Indexing 40%"},{"type":"list","items":[{"title":"Build #128","right":"3m","state":"doing","action":{"call":"build.open","params":{"id":128}}}]},{"type":"buttons","items":[{"label":"Refresh","action":{"call":"refresh"}}]}]
```

Text only: no Markdown, HTML or color codes. Full pages (`place = "page"`) can also hold editable forms (`sections`, `form`). Every section and form field is in [protocol reference: widgets](../reference/extensions.en.md#widgetset-widgetclear-widgetwatch-widgetunwatch).

**Being called by other packages, calling them**: list methods others may call in the manifest's `[export] methods` (still registered with `package.methods`); when another package calls, `method.call` carries `from` (the caller's package id, which can't be faked). To call others, declare `packages.call` and send `package.call {"package", "method", "params"}`.

**Reading content other packages bring for you**: another package writing `[content.<your id>] dir = "…"` in its manifest brings you a folder. Call `content.list {}` to see who did and `content.read {"package", "path", "offset"?}` to read in chunks; after installs, removals and upgrades the core sends `content.changed`.

**Recording your own events in a session**: `events.append {"session", "kind": "ext.<your id>.<name>", "body"}`. It doesn't start a turn and isn't shown to the model; UIs and your own later log reads see it.

## Don't

- Don't read or write elsewhere in the data root (sessions, other packages, config files, the secrets file): ask through the protocol.
- Methods for local people only (changing config, installing packages, `extension.*` and so on) are refused for extensions (`local_only`).
- **Declare capabilities honestly**: not everything is enforced yet, but undeclared ones will be blocked later.
