// 页探针（施工 MO-1，`internal/construction/MO-macOS的发行包排函数.md` 第 2 条「探针」）：macOS 上录排函数的清单用。
//
// 它是什么：一个动态库，量尺录清单时用 `DYLD_INSERT_LIBRARIES` 插进它自己起的被测进程。进 `main` 以前，它把主程序
// `__text` 节整页的部分设成不可访问；之后每有一个线程碰到其中一页，内核发 SIGBUS 或 SIGSEGV，这里的处理函数记下碰的
// 地址、放开这一页，被打断的指令重新执行。发出去的程序和发行包都不带这个变量，探针的动态库也不进发行包。
//
// 怎么编：`cc -O2 -dynamiclib -o pagetrace.dylib pagetrace.c`，要过 `-Wall -Wextra -Werror`（macOS 的 CI 编一次）。
// 只认 arm64；x86_64 读 pc 的写法用 `#if` 留着，不测。
//
// 环境变量：
//
// - `MIYU_PAGES_OUT=<前缀>`：没设的什么都不做。每个进程写 `<前缀>.<进程号>`（记录）和 `<前缀>.<进程号>.exe`（一行：可执行
//   文件的路径和前两个参数，用空格隔开）。主程序换成终端界面时进程号不变，探针在新的程序里重新初始化，这两份会被重写。
// - `MIYU_PAGES_PROGRAM=<路径>`：给了的，只在可执行文件的路径（两边都先解开符号链接）和它一样时才动手，别的进程载入了也
//   什么都不做，核心起的沙盒子进程不受干扰。
// - `MIYU_PAGES_REARM_MS=<毫秒>`：每隔这么久把已经放开的代码页重新全部关上，下一个碰到的会再记一次。没设、设成 0 的
//   不重关（每页只记得到第一个碰它的）。不是十进制整数、超过 60000 的，在标准错误上说一句、整个探针不动手。
//
// 记录文件的格式（和 `crates/miyu-perf/src/order/listing.rs` 的模块说明一致，改一边要改另一边）。都是小端的 64 位
// 无符号数。开头 8 个：
//
//   0  魔数，字节是 `MIYUPGT1`
//   1  `__TEXT` 段在符号表里的虚拟地址
//   2  页大小（字节）
//   3  `__text` 节在符号表里的起点
//   4  `__text` 节的大小（字节）
//   5  记录的上限
//   6  记录数（不超过上限）
//   7  标志：第 0 位是「满了」，别的位是 0
//
// 后面接着记录，每条两个数：碰的地址（已经减掉装载的滑动，是符号表里的地址）、种类（1 是执行，2 是读）。同一个地址只记
// 一次，照第一次见到的先后排。记录的上限是 `__text` 每 64 字节一条（最少 4096 条），满了就不再记，并在标志里标出来。
// 「执行」认的是碰的地址和 pc 在同一页：取指令时缺页，碰的地址就是 pc 所在的那一页；读别的代码页上的数据，pc 在一页
// 已经放开的代码上（或者在动态库里），碰的地址在另一页。
//
// 线程和信号：
//
// - 处理函数里只做异步信号安全的事：读写预先 mmap 好的两张表（记录、散列表），用 GCC 的 `__atomic` 内建函数（arm64
//   上都是无锁的），调 `mprotect`、`sigaction`、`write`。不 malloc、不加锁、不碰 stdio。
// - 去重用一张开放寻址的散列表，键是碰的地址（0 表示空：符号表里的地址不会是 0）。几个线程同时缺页时各自用比较并交换
//   占一个槽：占到的那一个再用原子加取一个记录的下标，先写地址、再用 release 写种类；落盘的线程用 acquire 读种类，从头
//   往后数到第一条种类还是 0 的为止，只写这之前的（还没写完的那几条下一次落盘再写），所以不会写出半条记录。
// - 重关和放行的竞争：线程 A 缺页、处理函数放开这一页，回去以前后台线程恰好又把全部代码页关上了。A 回去重新执行那条
//   指令时会再缺一次页，处理函数再放开一次；这个地址已经在表里，不再记。每一次缺页都由处理函数自己放开这一页，所以放行
//   不会丢；重关 20 毫秒一次、放行到重新执行只隔几微秒，不会一直撞上、转不出去。
// - 不是这里造成的缺页（地址不在关上的那一段、往放开的代码页上写、`mprotect` 失败）：把这个信号的处理恢复成系统默认的，
//   回去重新执行就照常崩溃，不会在处理函数里转圈。被测程序自己装 SIGBUS、SIGSEGV 处理函数的那一下被拦掉（下面的
//   `__interpose`，不然 Rust 的运行时会把这里的顶掉），所以录清单时 Rust 的栈溢出提示不起作用，栈溢出直接崩溃。
// - 落盘、重关都在一个后台线程里做（不靠 SIGUSR1：程序自己装了 SIGUSR1 的处理函数时会被顶掉）：每 300 毫秒先写临时文件
//   `<记录文件>.tmp` 再改名，量尺不会读到写了一半的文件。正常退出时（`exit`）再写一次；被信号杀掉的最多丢最后 300 毫秒。
// - 只关 `__text` 整页的部分，不关整个 `__TEXT` 段：核心起子进程时把 `__const`、`__cstring` 里的串直接交给内核，内核
//   碰到不可访问的页不发信号，而是让系统调用失败（EFAULT），核心报「无法启动核心：Bad address (os error 14)」。
#include <crt_externs.h>
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <mach-o/dyld.h>
#include <mach-o/loader.h>
#include <pthread.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/ucontext.h>
#include <time.h>
#include <unistd.h>

#define MAGIC 0x315447505559494dull /* 小端的字节是 "MIYUPGT1" */
#define KIND_EXEC 1u
#define KIND_READ 2u
#define FLAG_FULL 1u
#define BYTES_PER_RECORD 64u
#define MIN_RECORDS 4096u
#define DUMP_MS 300u
#define MAX_REARM_MS 60000u

struct record {
    uint64_t addr;
    uint64_t kind; /* 0 表示下标已经占了、还没写完 */
};

static char out_path[PATH_MAX + 32];
static char tmp_path[PATH_MAX + 40];
/* 关上的那一段（装载以后的地址）和滑动；armed 为 0 时处理函数、拦截都不动手。 */
static uintptr_t lo, hi, slide;
static volatile int armed;
static uint64_t text_vm, code_vm, code_len, page;
static unsigned rearm_ms;
static struct record *records;
static uint64_t capacity, count, flags;
static uint64_t *slots;
static uint64_t slot_mask;
/* 动手的那个进程：fork 出来、还没 exec 的子进程照样带着这些表，它退出时不能拿父进程的文件名写自己那一份。 */
static pid_t owner;

/* 不是这里造成的缺页：恢复系统默认的处理，回去重新执行那条指令就照常崩溃。 */
static void give_up(int sig) {
    struct sigaction sa;
    memset(&sa, 0, sizeof sa);
    sa.sa_handler = SIG_DFL;
    if (sigaction(sig, &sa, 0) != 0) {
        _exit(128 + sig);
    }
}

/* 这一次缺页的 pc，和它是不是一次写（往已经放开的代码页上写，不是这里关上造成的）。 */
static void fault_context(const ucontext_t *uc, uintptr_t *pc, int *writing) {
#if defined(__aarch64__)
    *pc = (uintptr_t)uc->uc_mcontext->__ss.__pc;
    uint32_t esr = uc->uc_mcontext->__es.__esr;
    uint32_t class = esr >> 26;
    /* 数据异常（EC 0x24、0x25）的第 6 位 WnR 是「写」。内核没填 ESR 时是 0，当成读。 */
    *writing = (class == 0x24 || class == 0x25) && (esr & (1u << 6)) != 0;
#elif defined(__x86_64__)
    /* 不测：只为留着写法。错误码的第 1 位是「写」。 */
    *pc = (uintptr_t)uc->uc_mcontext->__ss.__rip;
    *writing = (uc->uc_mcontext->__es.__err & 2u) != 0;
#else
#error "页探针只认 arm64（x86_64 留着写法）"
#endif
}

/* 记下一个地址：已经记过的、满了的不记。只用原子操作，可以在处理函数里、几个线程同时调。 */
static void note(uint64_t addr, uint64_t kind) {
    if (__atomic_load_n(&flags, __ATOMIC_RELAXED) & FLAG_FULL) {
        return;
    }
    uint64_t at = ((addr >> 2) * 0x9e3779b97f4a7c15ull) & slot_mask;
    for (uint64_t tries = 0; tries <= slot_mask; tries++, at = (at + 1) & slot_mask) {
        uint64_t seen = 0;
        if (!__atomic_compare_exchange_n(&slots[at], &seen, addr, 0, __ATOMIC_ACQ_REL, __ATOMIC_ACQUIRE)) {
            if (seen == addr) {
                return;
            }
            continue;
        }
        uint64_t index = __atomic_fetch_add(&count, 1, __ATOMIC_ACQ_REL);
        if (index >= capacity) {
            __atomic_fetch_or(&flags, FLAG_FULL, __ATOMIC_RELAXED);
            return;
        }
        records[index].addr = addr;
        __atomic_store_n(&records[index].kind, kind, __ATOMIC_RELEASE);
        return;
    }
    /* 散列表是记录上限的两倍以上，转一整圈都没有空槽说明满了。 */
    __atomic_fetch_or(&flags, FLAG_FULL, __ATOMIC_RELAXED);
}

static void fault(int sig, siginfo_t *info, void *raw) {
    uintptr_t addr = (uintptr_t)info->si_addr;
    uintptr_t pc;
    int writing;
    fault_context((const ucontext_t *)raw, &pc, &writing);
    if (!armed || addr < lo || addr >= hi || writing) {
        give_up(sig);
        return;
    }
    uintptr_t base = addr & ~(uintptr_t)(page - 1);
    if (mprotect((void *)base, (size_t)page, PROT_READ | PROT_EXEC) != 0) {
        static const char why[] = "页探针：放不开代码页，交回系统默认的处理\n";
        if (write(STDERR_FILENO, why, sizeof why - 1) < 0) {
            /* 标准错误也写不进时没有别的地方可说，照样交回默认的处理。 */
        }
        give_up(sig);
        return;
    }
    uint64_t kind = (pc & ~(uintptr_t)(page - 1)) == base ? KIND_EXEC : KIND_READ;
    note((uint64_t)(addr - slide), kind);
}

/* 把 `chunks` 依次写进 `path`：先写 `tmp`，写完再改名。哪一步不成就放弃这一次（下一次落盘再试）。 */
static void publish(const char *path, const char *tmp, const void *const *chunks, const size_t *sizes, int n) {
    int fd = open(tmp, O_WRONLY | O_CREAT | O_TRUNC | O_CLOEXEC, 0644);
    if (fd < 0) {
        return;
    }
    for (int i = 0; i < n; i++) {
        const char *at = chunks[i];
        size_t left = sizes[i];
        while (left > 0) {
            ssize_t done = write(fd, at, left);
            if (done < 0 && errno == EINTR) {
                continue;
            }
            if (done <= 0) {
                close(fd);
                unlink(tmp);
                return;
            }
            at += done;
            left -= (size_t)done;
        }
    }
    if (close(fd) != 0 || rename(tmp, path) != 0) {
        unlink(tmp);
    }
}

/* 写一次记录文件：只写从头数起已经写完的那些记录。 */
static void dump(void) {
    if (getpid() != owner) {
        return;
    }
    uint64_t reserved = __atomic_load_n(&count, __ATOMIC_ACQUIRE);
    uint64_t n = reserved < capacity ? reserved : capacity;
    uint64_t done = 0;
    while (done < n && __atomic_load_n(&records[done].kind, __ATOMIC_ACQUIRE) != 0) {
        done++;
    }
    uint64_t head[8] = {MAGIC, text_vm, page, code_vm, code_len, capacity, done,
                        __atomic_load_n(&flags, __ATOMIC_RELAXED) & FLAG_FULL};
    const void *chunks[2] = {head, records};
    size_t sizes[2] = {sizeof head, (size_t)done * sizeof(struct record)};
    publish(out_path, tmp_path, chunks, sizes, 2);
}

static void pause_ms(unsigned ms) {
    struct timespec left = {ms / 1000u, (long)(ms % 1000u) * 1000000L};
    while (nanosleep(&left, &left) != 0 && errno == EINTR) {
    }
}

/* 后台线程：每 `rearm_ms` 重关一次（设了的话），每 300 毫秒左右落一次盘。 */
static void *ticker(void *unused) {
    (void)unused;
    unsigned step = rearm_ms ? rearm_ms : DUMP_MS;
    unsigned since = 0;
    for (;;) {
        pause_ms(step);
        if (rearm_ms && mprotect((void *)lo, hi - lo, PROT_NONE) != 0) {
            static const char why[] = "页探针：重新关上代码页失败，之后不再重关\n";
            if (write(STDERR_FILENO, why, sizeof why - 1) < 0) {
                /* 没有别的地方可说。 */
            }
            rearm_ms = 0;
            step = DUMP_MS;
        }
        since += step;
        if (since >= DUMP_MS) {
            since = 0;
            dump();
        }
    }
    return 0;
}

/* 被测程序给 SIGBUS、SIGSEGV 装处理函数的那一下：探针动手了的就拦掉，交回「原来是默认的处理」。 */
static int probe_sigaction(int sig, const struct sigaction *act, struct sigaction *old) {
    if ((sig == SIGBUS || sig == SIGSEGV) && act && armed) {
        if (old) {
            memset(old, 0, sizeof *old);
        }
        return 0;
    }
    return sigaction(sig, act, old);
}

__attribute__((used, section("__DATA,__interpose"))) static const struct {
    const void *replacement;
    const void *original;
} interpose = {(const void *)probe_sigaction, (const void *)sigaction};

/* 一张 `bytes` 字节、全是 0 的表（匿名映射，没碰过的页不占内存）。 */
static void *table(uint64_t bytes) {
    void *at = mmap(0, (size_t)bytes, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANON, -1, 0);
    return at == MAP_FAILED ? 0 : at;
}

/* `MIYU_PAGES_PROGRAM` 没给，或者给的就是这个程序。两边都解不开的照原样比。 */
static int is_target(const char *exe) {
    const char *want = getenv("MIYU_PAGES_PROGRAM");
    if (!want || !*want) {
        return 1;
    }
    char a[PATH_MAX], b[PATH_MAX];
    const char *left = realpath(exe, a) ? a : exe;
    const char *right = realpath(want, b) ? b : want;
    return strcmp(left, right) == 0;
}

/* 读 `MIYU_PAGES_REARM_MS`：没设的是 0；不认得的交回 -1。 */
static int read_rearm(unsigned *ms) {
    const char *text = getenv("MIYU_PAGES_REARM_MS");
    *ms = 0;
    if (!text || !*text) {
        return 0;
    }
    char *end;
    errno = 0;
    unsigned long value = strtoul(text, &end, 10);
    if (errno || *end || text[0] == '-' || text[0] == '+' || value > MAX_REARM_MS) {
        return -1;
    }
    *ms = (unsigned)value;
    return 0;
}

/* 旁注：可执行文件的路径和前两个参数，一行。 */
static void side_note(const char *exe) {
    char line[PATH_MAX * 3];
    int used = snprintf(line, sizeof line, "%s", exe);
    char **argv = *_NSGetArgv();
    int argc = *_NSGetArgc();
    for (int k = 1; k < argc && k < 3 && used >= 0 && (size_t)used < sizeof line; k++) {
        used += snprintf(line + used, sizeof line - (size_t)used, " %s", argv[k]);
    }
    if (used < 0) {
        return;
    }
    size_t len = (size_t)used < sizeof line - 1 ? (size_t)used : sizeof line - 2;
    line[len] = '\n';
    char side[sizeof out_path + 8], side_tmp[sizeof out_path + 16];
    snprintf(side, sizeof side, "%s.exe", out_path);
    snprintf(side_tmp, sizeof side_tmp, "%s.exe.tmp", out_path);
    const void *chunks[1] = {line};
    size_t sizes[1] = {len + 1};
    publish(side, side_tmp, chunks, sizes, 1);
}

/* 在主程序的装载命令里找 `__TEXT` 段和它的 `__text` 节（`getsegbyname` 这一类从 macOS 13 起不推荐用了）。 */
static void find_text(const struct mach_header_64 *mh, const struct segment_command_64 **seg,
                      const struct section_64 **code) {
    const struct load_command *lc = (const struct load_command *)(mh + 1);
    for (uint32_t i = 0; i < mh->ncmds; i++, lc = (const struct load_command *)((const char *)lc + lc->cmdsize)) {
        if (lc->cmd != LC_SEGMENT_64) {
            continue;
        }
        const struct segment_command_64 *one = (const struct segment_command_64 *)lc;
        if (strcmp(one->segname, "__TEXT") != 0) {
            continue;
        }
        *seg = one;
        const struct section_64 *sections = (const struct section_64 *)(one + 1);
        for (uint32_t j = 0; j < one->nsects; j++) {
            if (strcmp(sections[j].sectname, "__text") == 0) {
                *code = &sections[j];
            }
        }
    }
}

__attribute__((constructor)) static void setup(void) {
    const char *prefix = getenv("MIYU_PAGES_OUT");
    if (!prefix || !*prefix) {
        return;
    }
    char exe[PATH_MAX];
    uint32_t size = sizeof exe;
    if (_NSGetExecutablePath(exe, &size) != 0 || !is_target(exe)) {
        return;
    }
    if (read_rearm(&rearm_ms) != 0) {
        static const char why[] = "页探针：MIYU_PAGES_REARM_MS 要写 0 到 60000 的毫秒数，这一次不动手\n";
        if (write(STDERR_FILENO, why, sizeof why - 1) < 0) {
            /* 没有别的地方可说。 */
        }
        return;
    }
    owner = getpid();
    if (snprintf(out_path, sizeof out_path, "%s.%d", prefix, (int)owner) >= (int)sizeof out_path ||
        snprintf(tmp_path, sizeof tmp_path, "%s.tmp", out_path) >= (int)sizeof tmp_path) {
        return;
    }
    const struct mach_header_64 *mh = (const struct mach_header_64 *)_NSGetMachExecuteHeader();
    const struct segment_command_64 *seg = 0;
    const struct section_64 *code = 0;
    find_text(mh, &seg, &code);
    if (!seg || !code) {
        return;
    }
    page = (uint64_t)getpagesize();
    text_vm = seg->vmaddr;
    code_vm = code->addr;
    code_len = code->size;
    slide = (uintptr_t)mh - (uintptr_t)seg->vmaddr;
    lo = (uintptr_t)((code_vm + slide + page - 1) & ~(page - 1));
    hi = (uintptr_t)((code_vm + slide + code_len) & ~(page - 1));
    capacity = code_len / BYTES_PER_RECORD;
    if (capacity < MIN_RECORDS) {
        capacity = MIN_RECORDS;
    }
    uint64_t width = 1;
    while (width < capacity * 2) {
        width <<= 1;
    }
    slot_mask = width - 1;
    records = table(capacity * sizeof(struct record));
    slots = table(width * sizeof(uint64_t));
    if (!records || !slots || hi <= lo) {
        return;
    }
    side_note(exe);
    dump();
    struct sigaction sa;
    memset(&sa, 0, sizeof sa);
    sa.sa_sigaction = fault;
    sa.sa_flags = SA_SIGINFO | SA_NODEFER;
    sigfillset(&sa.sa_mask);
    if (sigaction(SIGBUS, &sa, 0) != 0 || sigaction(SIGSEGV, &sa, 0) != 0) {
        return;
    }
    pthread_t thread;
    if (pthread_create(&thread, 0, ticker, 0) != 0) {
        return;
    }
    pthread_detach(thread);
    armed = 1;
    if (mprotect((void *)lo, hi - lo, PROT_NONE) != 0) {
        armed = 0;
    }
}

__attribute__((destructor)) static void finish(void) {
    if (armed) {
        dump();
    }
}
