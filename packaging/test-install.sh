#!/bin/sh
# install.sh 的实测（施工 RL-3）：在一个干净的家目录里装、核、卸一遍，资产用公开的预发布。CI 的 install-test.yml 和容器里都跑它，
# 别在自己的家目录跑（会装进 ~/.local、卸的时候停核心）。
#
#   sh packaging/test-install.sh <install.sh 的路径>
#
# 查的：没有正式版时不加 --prerelease 停下、退出码 1；--prerelease 装上，链接、install.toml 在，miyu --version 对得上，
# miyu upgrade --prerelease 说已是最新；再装说已经装了；有 python3 的另起一个本机的下载处，SHA256SUMS 被改过的（openssl 认 Ed25519
# 时）、压缩包被改过的都停下、什么都不装；卸掉以后安装目录、链接没了，数据根还在。
set -eu

SCRIPT="$1"
LANG=C
LC_ALL=C
export LANG LC_ALL

fail() {
  printf 'FAIL: %s\n' "$1" >&2
  exit 1
}

# 正式版还没有的时候，不加 --prerelease 的停下。有了以后这一条照装上算过。
if out=$(sh "${SCRIPT}" 2>&1); then
  printf '%s\n' "${out}" | grep -q 'Installed Miyu' || fail "装上了却没说：${out}"
  sh "${SCRIPT}" --uninstall
else
  printf '%s\n' "${out}" | grep -q 'no stable release yet' || fail "没有正式版时该说加 --prerelease：${out}"
fi
[ ! -e "${HOME}/.local/lib/miyu" ] || fail "停下的不该留东西"

sh "${SCRIPT}" --prerelease
[ -L "${HOME}/.local/bin/miyu" ] || fail "没有链接"
grep -q '^channel = "script"' "${HOME}/.local/lib/miyu/install.toml" || fail "install.toml 不对"
version=$("${HOME}/.local/bin/miyu" --version)
printf 'installed: %s\n' "${version}"
for left in "${HOME}"/.local/lib/.miyu-install.*; do
  [ ! -e "${left}" ] || fail "暂存没删：${left}"
done
# miyu upgrade 匿名问 GitHub：共用出口的 CI 机器上撞了限流的，记一句没验成，别的照查。
if up=$("${HOME}/.local/bin/miyu" upgrade --prerelease 2>&1); then
  printf '%s\n' "${up}" | grep -q 'already up to date' || fail "装完 miyu upgrade 该说已是最新：${up}"
else
  printf '%s\n' "${up}" | grep -q 'rate limit' || fail "miyu upgrade --prerelease 没成：${up}"
  echo "WARNING: miyu upgrade hit GitHub's anonymous rate limit; the up-to-date check wasn't verified on this run"
fi
again=$(sh "${SCRIPT}" --prerelease 2>&1) || fail "再装不该报错：${again}"
printf '%s\n' "${again}" | grep -q 'already installed' || fail "再装该说已经装了：${again}"

mkdir -p "${HOME}/.miyu"
echo keep > "${HOME}/.miyu/keep"

if command -v python3 >/dev/null 2>&1; then
  tag="v${version#miyu }"
  case "$(uname -s)-$(uname -m)" in
    Linux-x86_64) t=linux-x86_64 ;;
    Linux-aarch64) t=linux-aarch64 ;;
    *) t=macos-aarch64 ;;
  esac
  archive="miyu-${version#miyu }-${t}.tar.gz"
  srv=$(mktemp -d)
  mkdir -p "${srv}/${tag}"
  for f in SHA256SUMS SHA256SUMS.sig "${archive}"; do
    curl -fsSL -o "${srv}/${tag}/${f}" "https://github.com/miyu-agent/miyu/releases/download/${tag}/${f}"
  done
  port=$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1])')
  python3 -m http.server "${port}" --bind 127.0.0.1 --directory "${srv}" >/dev/null 2>&1 &
  server=$!
  sleep 1
  sh "${SCRIPT}" --uninstall >/dev/null
  # 压缩包被改过：哪台都该停在 SHA-256。
  cp "${srv}/${tag}/${archive}" "${srv}/good"
  printf 'x' >> "${srv}/${tag}/${archive}"
  out=$(MIYU_INSTALL_BASE="http://127.0.0.1:${port}" sh "${SCRIPT}" --version "${tag}" 2>&1) && fail "压缩包改过的装上了：${out}"
  printf '%s\n' "${out}" | grep -q 'SHA-256 of' || fail "该停在 SHA-256：${out}"
  mv "${srv}/good" "${srv}/${tag}/${archive}"
  # SHA256SUMS 被改过：openssl 认 Ed25519 的停在签名；不认的说核不了（这一条照 SHA-256 本身还对，会装上，再卸掉）。
  printf '0000  extra\n' >> "${srv}/${tag}/SHA256SUMS"
  if out=$(MIYU_INSTALL_BASE="http://127.0.0.1:${port}" sh "${SCRIPT}" --version "${tag}" 2>&1); then
    printf '%s\n' "${out}" | grep -q "doesn't support Ed25519" || fail "签名没核却没说：${out}"
    sh "${SCRIPT}" --uninstall >/dev/null
  else
    printf '%s\n' "${out}" | grep -q "signature on SHA256SUMS doesn't match" || fail "该停在签名：${out}"
  fi
  [ ! -e "${HOME}/.local/lib/miyu" ] || fail "停下的不该留东西"
  kill "${server}"
  rm -rf "${srv}"
  sh "${SCRIPT}" --prerelease >/dev/null
else
  echo "no python3: skipping the tampered downloads"
fi

sh "${SCRIPT}" --uninstall
[ ! -e "${HOME}/.local/lib/miyu" ] || fail "安装目录还在"
[ ! -e "${HOME}/.local/bin/miyu" ] && [ ! -L "${HOME}/.local/bin/miyu" ] || fail "链接还在"
[ -f "${HOME}/.miyu/keep" ] || fail "数据根被动了"
again=$(sh "${SCRIPT}" --uninstall 2>&1)
printf '%s\n' "${again}" | grep -q 'Not installed' || fail "再卸该说没装：${again}"
echo "install.sh: all checks passed"
