---
name: package-creator
description: "Write a Miyu package with scripts as new tools, or MCP servers. Use when asked to make a tool, a script or a package (写个工具、写脚本、做个包)."
metadata:
  display_name: Write a package
  display_name_zh: 写脚本和包
  summary: Turn scripts into tools in a package, check it, install it
  summary_zh: 把脚本做成工具放进包里，检查后装上
---

# Write a package

A package folder in the workspace, installed with the `package` tool, can carry skills (see skill-creator), scripts and MCP servers. Each script becomes one tool.

## Layout

```text
<package-id>/
├── package.toml
└── scripts/
    └── dice.py
```

```toml
[package]
name = { en = "Dice", zh = "掷骰子" }
```

The folder name is the package id. Do not write into the Miyu data folder: it is closed to file tools.

## A script

The tool name is the file name without its extension. The comment lines at the top describe it:

```python
#!/usr/bin/env python3
# Description: Roll dice and return each face and the total.
# Display name: Dice
# 显示名称：掷骰子
# Timeout: 30
# Parameters:
# {"type": "object", "properties": {"count": {"type": "integer"}, "sides": {"type": "integer"}}, "required": ["count", "sides"]}
import json, os, random
args = json.loads(os.environ["MIYU_ARGS_JSON"])
faces = [random.randint(1, args["sides"]) for _ in range(args["count"])]
print(json.dumps({"faces": faces, "total": sum(faces)}))
```

- `Description`: English, one or two short sentences; it is sent with every request.
- `Parameters`: a JSON Schema object; it may span several comment lines. Leave it out for no parameters.
- `Timeout`: seconds, 60 if left out. `Display name`, `显示名称`, `描述` are for people.
- Arguments come as JSON in `MIYU_ARGS_JSON` and on standard input. Standard output is the result; a non-zero exit code is an error.
- It runs like a command in the sandbox: it can write only in the workspace and `MIYU_SCRIPT_CACHE_DIR`. Use only what the machine already has (python3, sh); say in `Description` if it needs more.
- Start with `#!`. Run it once with `shell` before packaging it.

## MCP servers

Add one `[[mcp]]` table per server:

```toml
[[mcp]]
name = "files"
command = ["npx", "-y", "@modelcontextprotocol/server-filesystem", "/srv/notes"]
```

A package with MCP servers is installed off; the person turns it on in the package page.

## Check and install

Run these in a shell command:

1. `miyu pkg info ./<folder>`: lists the scripts it will add; anything under `skipped` will not load, fix it first.
2. `miyu pkg install ./<folder>`. The person is asked first. New tools appear from the next turn.
3. To change it, edit the files and install the folder again.
