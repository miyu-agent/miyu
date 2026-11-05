#!/bin/sh
# Miyu 的安装脚本，Linux、macOS 用（施工 RL-3，设计 59 第三节第 2 条、第四节）。Windows 用 install.ps1。
#
#   curl -fsSL https://raw.githubusercontent.com/miyu-agent/miyu/main/packaging/install.sh | sh
#   curl -fsSL …/install.sh | sh -s -- --prerelease      # 也算预发布
#   curl -fsSL …/install.sh | sh -s -- --version 1.0.0   # 装指定的一版
#   curl -fsSL …/install.sh | sh -s -- --uninstall       # 卸
#
# 从 GitHub Releases 下载这一种系统的压缩包，先用写在这里的发版公钥核 SHA256SUMS 的签名（openssl 认 Ed25519 的才核得了），
# 再核压缩包的 SHA-256，解开后跑一次新的 `miyu --version`。都对了才装：
#   - 压缩包第一层整个放进 ~/.local/lib/miyu/，旁边写 install.toml（channel = "script"，`miyu upgrade` 照它自己升级）；
#   - ~/.local/bin/miyu 链过去；PATH 里没有 ~/.local/bin 的只说怎么加，不改 shell 的配置；
#   - Linux 上装 .miyu 包文件的类型（~/.local/share/mime/packages/miyu.xml）。
# 不要管理员、不用 sudo。数据根（~/.miyu）装、卸都不碰。
#
# 设了 GITHUB_TOKEN 的，问 GitHub API 时带上它（只发给 API，下载不带），免得共用出口的机器撞上匿名每小时 60 次的限流。
# 测试用的两个环境变量：MIYU_INSTALL_API（GitHub 的 API 地址）、MIYU_INSTALL_BASE（下载地址的前半）。
set -eu

REPO="miyu-agent/miyu"
API="${MIYU_INSTALL_API:-https://api.github.com/repos/${REPO}}"
BASE="${MIYU_INSTALL_BASE:-https://github.com/${REPO}/releases/download}"
LIB="${HOME}/.local/lib/miyu"
BIN="${HOME}/.local/bin"
MIME="${XDG_DATA_HOME:-${HOME}/.local/share}/mime"

# 发版公钥：编号和 32 字节的 Ed25519 公钥（base64），同 crates/miyu-source/src/sign.rs 的 RELEASE（测试守着两边一样）。
KEY_ID="ca6918041857843c"
KEY="LOj3CWiaD/hcIp5yRUzy6zAGKivZDP5Dl+5O51kupYs="

# .miyu 包文件的类型，同 packaging/mime/miyu.xml（测试守着两边一样）。
MIME_XML='<?xml version="1.0" encoding="UTF-8"?>
<mime-info xmlns="http://www.freedesktop.org/standards/shared-mime-info">
  <mime-type type="application/x-zstd-compressed-tar">
    <glob pattern="*.miyu" weight="80"/>
  </mime-type>
</mime-info>'

case "${LC_ALL:-${LC_MESSAGES:-${LANG:-}}}" in
  zh*) ZH=1 ;;
  *) ZH=0 ;;
esac

# say <中文> <英文>：照语言挑一句印出来。
say() {
  if [ "${ZH}" = 1 ]; then printf '%s\n' "$1"; else printf '%s\n' "$2"; fi
}

# die <中文> <英文>：印到标准错误，退出码 1。
die() {
  if [ "${ZH}" = 1 ]; then printf 'miyu 安装：%s\n' "$1" >&2; else printf 'miyu install: %s\n' "$2" >&2; fi
  exit 1
}

usage() {
  say "用法：install.sh [--prerelease] [--version <版本>] [--uninstall]" \
      "Usage: install.sh [--prerelease] [--version <version>] [--uninstall]"
}

WANT=""
PRE=0
UNINSTALL=0
while [ $# -gt 0 ]; do
  case "$1" in
    --prerelease) PRE=1 ;;
    --uninstall) UNINSTALL=1 ;;
    --version)
      [ $# -ge 2 ] || die "--version 后面要写版本" "--version needs a version"
      WANT="$2"
      shift
      ;;
    --version=*) WANT="${1#--version=}" ;;
    -h | --help) usage; exit 0 ;;
    *) usage >&2; exit 1 ;;
  esac
  shift
done
WANT="${WANT#v}"

# 这台是哪一种：同 `cargo xtask dist` 的名字。
target() {
  os=$(uname -s)
  arch=$(uname -m)
  case "${os}-${arch}" in
    Linux-x86_64 | Linux-amd64) echo "linux-x86_64" ;;
    Linux-aarch64 | Linux-arm64) echo "linux-aarch64" ;;
    Darwin-arm64 | Darwin-aarch64) echo "macos-aarch64" ;;
    *) die "没有 ${os} ${arch} 的发行包（有 Linux x86_64、aarch64，macOS Apple 芯片）" \
           "no release for ${os} ${arch} (there are Linux x86_64 and aarch64, and macOS on Apple silicon)" ;;
  esac
}

sha256() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -d ' ' -f 1
  else
    shasum -a 256 "$1" | cut -d ' ' -f 1
  fi
}

# 卸：先停服务、停核心，再删安装目录、链接、文件类型。数据根不碰。
uninstall() {
  if [ ! -e "${LIB}" ]; then
    say "没有装着（${LIB} 不在）" "Not installed (${LIB} doesn't exist)"
    exit 0
  fi
  grep -q '^channel = "script"' "${LIB}/install.toml" 2>/dev/null \
    || die "${LIB} 不是这个脚本装的（没有 install.toml），不动它" \
           "${LIB} wasn't installed by this script (no install.toml); leaving it alone"
  if [ -x "${LIB}/miyu" ]; then
    "${LIB}/miyu" service disable >/dev/null 2>&1 \
      || say "开机启动的服务没拿掉，接着卸（之后自己 miyu service disable 不了的，照 packaging/README.md 手动删）" \
             "Couldn't remove the startup service; going on (remove it by hand as in packaging/README.md)"
    "${LIB}/miyu" core stop >/dev/null 2>&1 \
      || say "核心没停下，接着卸" "Couldn't stop the core; going on"
  fi
  rm -rf "${LIB}"
  if [ -L "${BIN}/miyu" ] && [ "$(readlink "${BIN}/miyu")" = "${LIB}/miyu" ]; then
    rm -f "${BIN}/miyu"
  fi
  if [ -f "${MIME}/packages/miyu.xml" ]; then
    rm -f "${MIME}/packages/miyu.xml"
    if command -v update-mime-database >/dev/null 2>&1; then
      update-mime-database "${MIME}" >/dev/null 2>&1 || true
    fi
  fi
  say "已卸载 Miyu。数据还在 ${MIYU_HOME:-${HOME}/.miyu}，不要了自己删。" \
      "Miyu is uninstalled. Your data is still in ${MIYU_HOME:-${HOME}/.miyu}; delete it yourself if you don't need it."
}

# 照 API 地址的回应找标签；没有的空。$1 是 latest 或 list。
find_tag() {
  answer="${WORK}/release.json"
  if [ "$1" = latest ]; then url="${API}/releases/latest"; else url="${API}/releases?per_page=30"; fi
  if [ -n "${GITHUB_TOKEN:-}" ]; then
    code=$(curl -sS -L -o "${answer}" -w '%{http_code}' -H 'Accept: application/vnd.github+json' \
      -H "Authorization: Bearer ${GITHUB_TOKEN}" "${url}") || die "连不上 GitHub（${url}）" "can't reach GitHub (${url})"
  else
    code=$(curl -sS -L -o "${answer}" -w '%{http_code}' -H 'Accept: application/vnd.github+json' "${url}") \
      || die "连不上 GitHub（${url}）" "can't reach GitHub (${url})"
  fi
  case "${code}" in
    200) ;;
    404) return 0 ;;
    403 | 429) die "GitHub 限流了（匿名每小时 60 次），过一会儿再试" \
                  "GitHub rate limit reached (60 an hour without signing in); try again later" ;;
    *) die "GitHub 回了 ${code}（${url}）" "GitHub answered ${code} (${url})" ;;
  esac
  # 回应可能压成一行：照逗号拆开，第一个 tag_name 就是最新的（列表照新到旧排，匿名看不到草稿）。
  tr ',' '\n' < "${answer}" | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\(v[^"]*\)".*/\1/p' | head -n 1
}

# 下载 $1 到 $2；不成的停。
fetch() {
  curl -fsSL -o "$2" "$1" || die "下载不成：$1" "download failed: $1"
}

# 照写在这里的公钥核 SHA256SUMS 的签名。核得了却核不过的停；这台的 openssl 不认 Ed25519 的说一句、接着装。
verify_signature() {
  sums="$1"
  sig="$2"
  key_of_sig=$(tr ',' '\n' < "${sig}" | sed -n 's/.*"key"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)
  [ "${key_of_sig}" = "${KEY_ID}" ] \
    || die "SHA256SUMS 是别的钥匙签的（${key_of_sig}），不装" "SHA256SUMS is signed by another key (${key_of_sig}); not installing"
  if ! command -v openssl >/dev/null 2>&1; then
    say "这台没有 openssl，没法核签名；只核 SHA-256。要核签名：gh attestation verify <文件> -R ${REPO}" \
        "No openssl here, so the signature can't be checked; checking SHA-256 only. To check it: gh attestation verify <file> -R ${REPO}"
    return 0
  fi
  pem="${WORK}/release.pem"
  {
    echo "-----BEGIN PUBLIC KEY-----"
    # Ed25519 公钥的 DER 前缀（RFC 8410）加上 32 字节的公钥。
    { printf '\060\052\060\005\006\003\053\145\160\003\041\000'; printf '%s' "${KEY}" | openssl base64 -d -A; } | openssl base64 -A
    echo
    echo "-----END PUBLIC KEY-----"
  } > "${pem}"
  tr ',' '\n' < "${sig}" | sed -n 's/.*"signature"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1 \
    | openssl base64 -d -A > "${WORK}/sums.sig"
  # 认不认 Ed25519：读得了这把公钥、pkeyutl 有 -rawin（OpenSSL 3 起）。macOS 自带的 LibreSSL 两样都不行。
  if ! openssl pkey -pubin -in "${pem}" -noout >/dev/null 2>&1 \
    || ! openssl pkeyutl -help 2>&1 | grep -q -- '-rawin'; then
    say "这台的 openssl 不认 Ed25519，签名没核（不是签名坏了）；只核 SHA-256。要核签名：gh attestation verify <文件> -R ${REPO}" \
        "This machine's openssl doesn't support Ed25519, so the signature wasn't checked (it isn't broken); checking SHA-256 only. To check it: gh attestation verify <file> -R ${REPO}"
    return 0
  fi
  openssl pkeyutl -verify -pubin -inkey "${pem}" -rawin -in "${sums}" -sigfile "${WORK}/sums.sig" >/dev/null 2>&1 \
    || die "SHA256SUMS 的签名核不过，不装" "the signature on SHA256SUMS doesn't match; not installing"
  say "签名核过了（发版钥匙 ${KEY_ID}）" "Signature checked (release key ${KEY_ID})"
}

install() {
  command -v curl >/dev/null 2>&1 || die "要 curl" "curl is needed"
  command -v tar >/dev/null 2>&1 || die "要 tar" "tar is needed"
  t=$(target) || exit 1
  if [ -e "${LIB}/miyu" ]; then
    now=$("${LIB}/miyu" --version 2>/dev/null || echo miyu)
    say "已经装了 ${now}（${LIB}），升级用 miyu upgrade" "${now} is already installed (${LIB}); use miyu upgrade to upgrade"
    exit 0
  fi
  [ ! -e "${LIB}" ] || die "${LIB} 已经有了、里面却没有 miyu，先挪走它" "${LIB} exists but has no miyu in it; move it away first"
  if [ -e "${BIN}/miyu" ] || [ -L "${BIN}/miyu" ]; then
    [ -L "${BIN}/miyu" ] && [ "$(readlink "${BIN}/miyu")" = "${LIB}/miyu" ] \
      || die "${BIN}/miyu 已经有了（不是这个脚本放的），先挪走它" "${BIN}/miyu already exists (not placed by this script); move it away first"
  fi

  mkdir -p "${HOME}/.local/lib"
  # 暂存放在 ~/.local/lib 下：和装的地方同一块盘，最后一步只是改名。
  WORK=$(mktemp -d "${HOME}/.local/lib/.miyu-install.XXXXXX")
  trap 'rm -rf "${WORK}"' EXIT INT TERM

  if [ -n "${WANT}" ]; then
    tag="v${WANT}"
  elif [ "${PRE}" = 1 ]; then
    tag=$(find_tag list) || exit 1
    [ -n "${tag}" ] || die "没有找到可用的发行版" "no release found"
  else
    tag=$(find_tag latest) || exit 1
    [ -n "${tag}" ] || die "还没有正式版；要装预发布的加 --prerelease" "no stable release yet; add --prerelease to install a pre-release"
  fi
  version="${tag#v}"
  name="miyu-${version}-${t}"
  archive="${name}.tar.gz"

  say "正在下载 ${archive}" "Downloading ${archive}"
  fetch "${BASE}/${tag}/SHA256SUMS" "${WORK}/SHA256SUMS"
  fetch "${BASE}/${tag}/SHA256SUMS.sig" "${WORK}/SHA256SUMS.sig"
  verify_signature "${WORK}/SHA256SUMS" "${WORK}/SHA256SUMS.sig"
  want=$(awk -v f="${archive}" '$2 == f || $2 == "*" f { print $1 }' "${WORK}/SHA256SUMS")
  [ -n "${want}" ] || die "SHA256SUMS 里没有 ${archive}" "${archive} isn't listed in SHA256SUMS"
  fetch "${BASE}/${tag}/${archive}" "${WORK}/${archive}"
  got=$(sha256 "${WORK}/${archive}")
  [ "${got}" = "${want}" ] || die "${archive} 的 SHA-256 对不上，不装" "the SHA-256 of ${archive} doesn't match; not installing"

  mkdir "${WORK}/unpacked"
  tar -xzf "${WORK}/${archive}" -C "${WORK}/unpacked" || die "解不开 ${archive}" "can't unpack ${archive}"
  top="${WORK}/unpacked/${name}"
  [ -x "${top}/miyu" ] && [ -d "${top}/resources" ] || die "${archive} 里缺东西" "${archive} is missing files"
  reported=$("${top}/miyu" --version 2>/dev/null || true)
  [ "${reported}" = "miyu ${version}" ] \
    || die "新的 miyu --version 报的是「${reported}」，不是 ${version}" "the new miyu --version says \"${reported}\", not ${version}"

  printf '# 安装脚本装的（施工 RL-3）：miyu upgrade 照它自己下载、核对、替换。\nchannel = "script"\n' > "${top}/install.toml"
  mv "${top}" "${LIB}"
  mkdir -p "${BIN}"
  ln -sf "${LIB}/miyu" "${BIN}/miyu"
  if [ "${t#linux}" != "${t}" ]; then
    mkdir -p "${MIME}/packages"
    printf '%s\n' "${MIME_XML}" > "${MIME}/packages/miyu.xml"
    if command -v update-mime-database >/dev/null 2>&1; then
      update-mime-database "${MIME}" >/dev/null 2>&1 || true
    fi
  fi

  say "已安装 Miyu ${version}（${LIB}）" "Installed Miyu ${version} (${LIB})"
  case ":${PATH}:" in
    *":${BIN}:"*) ;;
    *) say "提示：${BIN} 不在 PATH 里，在 shell 的配置里加一行：export PATH=\"${BIN}:\$PATH\"" \
           "Note: ${BIN} isn't on PATH; add this line to your shell's config: export PATH=\"${BIN}:\$PATH\"" ;;
  esac
  other=$(command -v miyu 2>/dev/null || true)
  if [ -n "${other}" ] && [ "${other}" != "${BIN}/miyu" ]; then
    say "提示：PATH 里先找到的是 ${other}，不是刚装的这一份" "Note: ${other} comes first on PATH, not the one just installed"
  fi
  say "下一步：运行 miyu 开始；要开机就起，运行 miyu service enable。升级用 miyu upgrade，卸载用这个脚本加 --uninstall。" \
      "Next: run miyu to start; run miyu service enable to start it at login. Upgrade with miyu upgrade; uninstall with this script and --uninstall."
}

if [ "${UNINSTALL}" = 1 ]; then
  uninstall
else
  install
fi
