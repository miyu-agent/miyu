# Quick start

Chinese version: [中文](../../zh/guide/quickstart.md).

## Quick install

A quick install gives you the Miyu bundle: the core, the basic features, the terminal interface (`tui`), the web interface (`webui`) and the system tray (`applet`). On Mac and Linux it also installs the extremely light `litemiyu` as a spare and for emergencies.

> It is best to have two agents: when one breaks, there is still the other.

The installer differs by operating system, so pick the one you use

- Windows: download the `.msi` installer from the [releases page](https://github.com/miyu-agent/miyu-applet/releases).
- Linux: download the `.deb` or `.rpm` from the same releases page; or use one install command: `curl -fsSL https://raw.githubusercontent.com/miyu-agent/miyu/main/packaging/install.sh | sh`.
- MacOS: with Homebrew, run `brew tap miyu-agent/miyu`, `brew install miyu-agent` and `brew install --cask miyu-applet` in turn; or download the `.dmg` from the same releases page.
- Arch Linux can install from the AUR: `paru -S miyu-agent`.

For the details of each way, upgrading and uninstalling, see [Installing, upgrading and uninstalling](install.md).

### Optional: custom install

If you use Linux or MacOS, you can install only the core and then pick what to install with the `miyu pkg` command. For example, if you only want the web interface, use `miyu pkg install webui`; the package for the terminal interface is `tui`. Run `miyu pkg --help` to see how to use it.

## First launch

After a quick install, Miyu appears in your application menu. Once it is started you see the Miyu icon in the system tray; right-click the icon for the menu and choose `Open Web Interface` to open `webui`. If you want the TUI, run `miyu` or `miyu tui` in a terminal. If you installed only the core and `webui`, run `miyu web` in a terminal to open `webui`.

The first launch starts the first-run guide. After you click `Start`, it goes through these steps:

### Interface language

Decides which language the interface is shown in. With the first option, `Follow system (auto)`, selected, some content is still shown in English for visual reasons.

### Select a model provider

Set up an AI provider and pick a model; custom providers are supported.

###  Create persona

A "persona" is who the AI you talk to is, and it decides the AI's memory. Different sessions can use different personas.

Give your AI a `Name`, set an `Avatar`, and write a `Detailed prompt` that describes the AI in detail. To shape how the AI speaks, add a few pairs of `Example dialogue`; to keep the AI from drifting away from its persona in long conversations, use the `Reminder phrases` feature.

Of course, you can also skip this and use Miyu with `No persona`; memory still works as usual.

### How the AI knows you

Here you can describe yourself: set the `Reply language` you prefer and your `Name`, and use a prompt to describe yourself in detail or say what you want the AI to keep in mind.

Of course, you can skip this too.

### Select preset

A "preset" is which features are turned on. Miyu ships with some presets, and you can also create your own `Custom preset`. Each session can have its own preset, and you can switch presets in the middle of a session.

### Turn on the built-in semantic model?

The semantic model greatly improves retrieval, so memory and knowledge base searches get better results. Unless your machine is a twenty-year-old antique, turning it on is recommended.

### Open to the local network

`webui` can be opened to the local network, so other devices on the same network can reach the machine Miyu runs on. Together with a virtual networking tool such as `tailscale` or `easytier`, you can reach `webui` when you are away as well.

## Packages

Once the guide is finished you can use the basic features for agent work. Most of the basic features come as built-in packages, and you can install extra packages to add to what Miyu can do.

### Installing and removing

In `webui`, click the gear icon at the bottom of the left sidebar to open the settings page and go to the `Get software` page; in `tui`, run `/config` in the input box and go to the `Packages` page of the settings menu; you can also use the `miyu pkg` command, see `miyu pkg --help` for how.

 ### Changing settings

An extension package's settings are mainly in two places. Go to the installed packages page, find the installed package and open its details page to see its settings. A package can also bring its own dashboard; besides opening it from the package's details page, the `Dashboards` page of the settings menu collects these dashboards for quick access.

### Making your own extension

If you want to create and publish your own package, you can read: [Writing a Miyu package](../../authors/README.en.md), or start from the [package template](https://github.com/miyu-agent/miyu-package-template).

## Connecting chat platforms

Miyu's official package source includes a ready-to-use IM integration.

Install the `Connect Messaging` (`miyu-im`) package and open its dashboard. Besides opening the dashboard from `tui` or `webui`, you can run `miyu im web`. On the home page, click `New connection` to see the supported platforms and ways to connect, then follow the guide step by step. After creating a connection, click the `New` button on the `Chat settings` page to create a chat card, where you set how the AI behaves in each group chat/private chat.

## Tips

### Model pools

Miyu has three built-in tiers of model pools, and you can also define your own. If you have suitable models, setting up the `Lite pool` and the `Flagship pool` is recommended: the `Lite pool` is used by default to name sessions, and the `Flagship pool` can serve the `advisor` feature, which lets the AI ask a flagship model for help.

### Autonomous loops

A prompt that starts with `/loop` puts the AI into an autonomous loop: the AI works through the task the user gave it on its own, at a suitable pace.

