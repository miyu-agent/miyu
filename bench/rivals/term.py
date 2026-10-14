"""在伪终端里跑一个终端界面（施工 V-7）：像 kitty 那样应答它问终端的几样（光标位置、DA1、前景背景色、16 色、图形协议、
字格和窗口大小、键盘协议、同步输出），用 pyte 看屏幕。三家起来时都问终端，没人答的会卡在那里等。"""

import fcntl
import os
import pty
import re
import select
import signal
import struct
import termios
import time

import pyte

COLS, ROWS = 140, 40
PALETTE = [
    "000000",
    "cd3131",
    "0dbc79",
    "e5e510",
    "2472c8",
    "bc3fbc",
    "11a8cd",
    "e5e5e5",
    "666666",
    "f14c4c",
    "23d18b",
    "f5f543",
    "3b8eea",
    "d670d6",
    "29b8db",
    "ffffff",
]


def rgb(hexs):
    r, g, b = hexs[0:2], hexs[2:4], hexs[4:6]
    return f"rgb:{r}{r}/{g}{g}/{b}{b}"


class Term:
    def __init__(self, argv, env, cwd):
        self.screen = pyte.Screen(COLS, ROWS)
        self.stream = pyte.ByteStream(self.screen)
        self.raw = bytearray()
        self.first_byte = None
        self.started = time.monotonic()
        pid, fd = pty.fork()
        if pid == 0:
            os.chdir(cwd)
            os.execvpe(argv[0], argv, env)
        fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))
        self.pid, self.fd = pid, fd

    def answer(self, data):
        out = b""
        if b"\x1b[6n" in data:
            out += b"\x1b[1;1R"
        for m in re.finditer(rb"\x1b\]4;(\d+);\?(?:\x07|\x1b\\)", data):
            n = int(m.group(1))
            if n < 16:
                out += b"\x1b]4;%d;%s\x1b\\" % (n, rgb(PALETTE[n]).encode())
        if re.search(rb"\x1b\]10;\?", data):
            out += b"\x1b]10;" + rgb("e5e5e5").encode() + b"\x1b\\"
        if re.search(rb"\x1b\]11;\?", data):
            out += b"\x1b]11;" + rgb("1a1b26").encode() + b"\x1b\\"
        # 像 kitty 那样回答：图形协议、字格大小、窗口大小、状态报告、版本、键盘协议、同步输出。
        if re.search(rb"\x1b_Gi=(\d+)[^\x1b]*a=q", data):
            i = re.search(rb"\x1b_Gi=(\d+)", data).group(1)
            out += b"\x1b_Gi=" + i + b";OK\x1b\\"
        if b"\x1b[16t" in data:
            out += b"\x1b[6;20;10t"
        if b"\x1b[14t" in data:
            out += b"\x1b[4;%d;%dt" % (ROWS * 20, COLS * 10)
        if b"\x1b[18t" in data:
            out += b"\x1b[8;%d;%dt" % (ROWS, COLS)
        if b"\x1b[5n" in data:
            out += b"\x1b[0n"
        if b"\x1b[>q" in data or b"\x1b[>0q" in data:
            out += b"\x1bP>|kitty(0.40.1)\x1b\\"
        if b"\x1b[?u" in data:
            out += b"\x1b[?0u"
        for m in re.finditer(rb"\x1b\[\?(\d+)\$p", data):
            out += b"\x1b[?" + m.group(1) + b";2$y"
        if re.search(rb"\x1b\[0?c", data):
            out += b"\x1b[?62;22c"
        if out:
            os.write(self.fd, out)

    def read_once(self, timeout):
        """最多等 `timeout` 秒读一块：交回读到的时刻，没读到是 0，终端关了是 None。

        时刻在读到的当下记，后面回答、喂给 pyte 花的工夫不算在里面。"""
        r, _, _ = select.select([self.fd], [], [], timeout)
        if not r:
            return 0
        try:
            data = os.read(self.fd, 65536)
        except OSError:
            return None
        if not data:
            return None
        read_at = time.monotonic()
        if self.first_byte is None:
            self.first_byte = read_at
        self.raw += data
        self.answer(data)
        try:
            self.stream.feed(data)
        except Exception:  # pyte 不认的序列会抛；屏幕只用来找字，丢掉这一块不影响计时和计量。
            pass
        return read_at

    def pump(self, seconds):
        """接着读 `seconds` 秒；终端关了交回 False。"""
        end = time.monotonic() + seconds
        while (left := end - time.monotonic()) > 0:
            if self.read_once(min(left, 0.05)) is None:
                return False
        return True

    def text(self):
        rows = []
        for y in range(self.screen.lines):
            row = self.screen.buffer[y]
            rows.append("".join((row[x].data or " ") for x in range(self.screen.columns)))
        return "\n".join(rows)

    def flat(self):
        """屏幕上的字去掉所有空格：pyte 会在宽字之间插空格，比较时不看空格。"""
        return re.sub(r"\s+", "", self.text())

    def wait_for(self, markers, timeout):
        """等屏幕上出现 `markers` 里任何一个（去掉空格比）；交回读到那一块的时刻（单调时钟），等不到是 None。

        每读到一块就看一次屏幕：隔一段时间才看，到首屏的时间会多算最多那一段。"""
        if isinstance(markers, str):
            markers = [markers]
        wants = [re.sub(r"\s+", "", m) for m in markers]
        end = time.monotonic() + timeout
        while (left := end - time.monotonic()) > 0:
            read_at = self.read_once(min(left, 0.05))
            if read_at is None:
                return None
            if read_at and any(w in self.flat() for w in wants):
                return read_at
        return None

    def send(self, data):
        os.write(self.fd, data)

    def close(self):
        try:
            os.kill(self.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        self.pump(0.5)
        try:
            os.waitpid(self.pid, os.WNOHANG)
        except ChildProcessError:
            pass
