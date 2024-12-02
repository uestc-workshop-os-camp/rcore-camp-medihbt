# RCore OS Report 1: LibOS

By medihbt 唐学迅 [(GitHub)](https://github.com/medihbt)

## 课后练习

### 编程题

1. 实现一个linux应用程序A，显示当前目录下的文件名。

```C
#include <dirent.h>
#include <unistd.h>
#include <stdlib.h>
#include <stdio.h>
#include <stdbool.h>

/** 表示当前目录名称的缓冲区. */
static char cwd_buf[4096] = {0};

int main()
{
    char const* cwdname = getcwd(cwd_buf, sizeof(cwd_buf));
    if (cwdname == NULL) {
        fputs("[FATAL] Your current working directory name is too long to process!\n", stderr);
        abort();
    }

    DIR* cwd = opendir(cwdname);
    while (true) {
        struct dirent* entry = readdir(cwd);
        if (entry == NULL)
            break;
        puts(entry->d_name);
    }

    closedir(cwd);
    return 0;
}
```

2. 实现一个linux应用程序B，能打印出调用栈链信息. （C/Rust)

同时展示使用 GLibC 和使用 RISC-V ABI 的栈回溯方法. GLibC 可以直接显示符号名称.

```sh
gcc B.c -o B -rdynamic
```

使用 riscv 的栈回溯采用了栈帧的 -1(存储 ra) 与 -2(存储 fp) 偏移位置, 需要这么编译:

```sh
clang B.c -o B --target=riscv64-unknown-linux-gnu -static -fno-omit-frame-pointer
```

```C
#include <execinfo.h>
#include <stdlib.h>
#include <threads.h>
#include <unistd.h>
#include <stdint.h>
#include <stdio.h>

#ifdef __GNUC__
static void gnuc_print_backtrace()
{
    static thread_local void* backtrace_buffer[128] = { NULL };
    int pid = getpid();
    fprintf(stderr, "======== [BEGIN backtrace of process %d] ========\n", pid);
    int nlayers = backtrace(backtrace_buffer, 128);
    backtrace_symbols_fd(backtrace_buffer, nlayers, STDERR_FILENO);
    fprintf(stderr, "========  [END backtrace of process %d]  ========\n", pid);
}
#endif

#ifdef __riscv
typedef void (*TraceHandlerT)(void* ra, void* fp);
static void riscv_backtrace(TraceHandlerT handler)
{
    if (handler == NULL)
        return;
    fputs("======== [BEGIN RISC-V backtrace] ========\n", stderr);
    uintptr_t* fp;
    __asm("mv %0, fp": "=r"(fp)::);

    while (fp != NULL && (uintptr_t)fp > 0x10) {
        void* ra     = (void*)fp[-1];
        void* nextfp = (void*)fp[-2];
        handler(ra, fp);
        fp = nextfp;
    }
    fputs("========= [END RISC-V backtrace] =========\n", stderr);
}
static void print_addr(void* ra, void* fp) {
    fprintf(stderr, "ra = %p, fp = %p\n", ra, fp);
}
#endif

/** 看上去有些奇怪, 但是这只是一个简单的展示函数, 栈当然是摞得越高越好 */
[[gnu::noinline]]
size_t do_factorial(uint8_t n, uint8_t max) {
    if (n > 20) {
        fprintf(stderr, "n too large: requires less than 20, but got %hhu\n", max);
#   ifdef __GNUC__
        gnuc_print_backtrace();
#   endif
#   ifdef __riscv
        riscv_backtrace(print_addr);
#   endif
        abort();
    }
    if (n == max)   return n;
    return do_factorial(n + 1, max) * n;
}

/** 这里使用递归阶乘来展示符号栈 */
[[gnu::noinline]]
size_t factorial(uint8_t n) {
    return n == 0? 1: do_factorial(1, n);
}

int main(void)
{
    factorial(23);
}
```

3. 实现一个基于rcore/ucore tutorial的应用程序C，用sleep系统调用睡眠5秒（in rcore/ucore tutorial v3: Branch ch1）.

程序如下:

```rust
#![no_std]
#![no_main]

extern crate user_lib;

#![no_mangle]
pub fn main() -> i32 {
    user_lib::sleep(5000);
    0
}
```

### 问答题

1. **应用程序在执行过程中，会占用哪些计算机资源?**

> 应用程序在执行过程中会占用：处理器(时间)资源、存储器(空间)资源、外设资源等等.

2. **请用相关工具软件分析并给出应用程序A的代码段/数据段/堆/栈的地址空间范围。**

使用 gdb 调试该可执行文件. 假定程序 A 是 x86_64-unknown-linux-gnu 平台的, 那可以得到这样的结果:

```gdb
`/home/medihbt/Devel/rcore/ch1/hw/ls', file type elf64-x86-64.
Entry point: 0x1120
0x0318 - 0x0334 is .interp
0x0338 - 0x0368 is .note.gnu.property
0x0368 - 0x038c is .note.gnu.build-id
0x038c - 0x03ac is .note.ABI-tag
0x03b0 - 0x03d8 is .gnu.hash
0x03d8 - 0x0528 is .dynsym
0x0528 - 0x05e9 is .dynstr
0x05ea - 0x0606 is .gnu.version
0x0608 - 0x0638 is .gnu.version_r
0x0638 - 0x0710 is .rela.dyn
0x0710 - 0x07b8 is .rela.plt
0x1000 - 0x101b is .init
0x1020 - 0x10a0 is .plt
0x10a0 - 0x10b0 is .plt.got
0x10b0 - 0x1120 is .plt.sec
0x1120 - 0x12a9 is .text
0x12ac - 0x12b9 is .fini
0x2000 - 0x204d is .rodata
0x2050 - 0x2084 is .eh_frame_hdr
0x2088 - 0x2134 is .eh_frame
0x3d88 - 0x3d90 is .init_array
0x3d90 - 0x3d98 is .fini_array
0x3d98 - 0x3f88 is .dynamic
0x3f88 - 0x4000 is .got
0x4000 - 0x4010 is .data
0x4020 - 0x5040 is .bss
```

- 代码段（权限为可读可执行的）:
  - `.init` 段地址范围 `[0x1000, 0x101b]`
  - `.text` 段地址范围 `[0x1120, 0x12a9]`
  - `.fini` 段地址范围 `[0x12ac, 0x12b9]`
  - `.rodata` 段地址范围 `[0x2000, 0x204d]`
- 数据段（权限为读写的）:
  - `.data` 段地址范围 `[0x4000, 0x4010]`
  - `.bss` 段地址范围 `[0x4020, 0x5040]`

接下来运行应用程序, 找到堆区和栈区地址.

```
(gdb) break _start
(gdb) break main
(gdb) run
Breakpoint 2.2, 0x00007ffff7fe4540 in _start () from /lib64/ld-linux-x86-64.so.2
(gdb) disassemble
Dump of assembler code for function _start:
=> 0x00007ffff7fe4540 <+0>:     mov    %rsp,%rdi
   0x00007ffff7fe4543 <+3>:     call   0x7ffff7fe51d0 <_dl_start>
(gdb) info registers rsp
rsp            0x7fffffffdae0      0x7fffffffdae0
(gdb) c
```

使用 pmap 查看程序的地址映射, 得到:

```
$ pmap -x 10565
10565:   /home/medihbt/Devel/rcore/ch1/hw/ls
Address           Kbytes     RSS   Dirty Mode  Mapping
0000555555554000       4       4       0 r---- ls
0000555555555000       4       4       4 r-x-- ls
0000555555556000       4       0       0 r---- ls
0000555555557000       4       4       4 r---- ls
0000555555558000       4       4       4 rw--- ls
0000555555559000       4       0       0 rw---   [ anon ]
00007ffff7c00000     160     156       0 r---- libc.so.6
00007ffff7c28000    1568     672       0 r-x-- libc.so.6
00007ffff7db0000     316     128       0 r---- libc.so.6
00007ffff7dff000      16      16      16 r---- libc.so.6
00007ffff7e03000       8       8       8 rw--- libc.so.6
00007ffff7e05000      52      16      16 rw---   [ anon ]
00007ffff7fa3000      12       8       8 rw---   [ anon ]
00007ffff7fbd000       8       4       4 rw---   [ anon ]
00007ffff7fbf000      16       0       0 r----   [ anon ]
00007ffff7fc3000       8       8       0 r-x--   [ anon ]
00007ffff7fc5000       4       4       0 r---- ld-linux-x86-64.so.2
00007ffff7fc6000     172     168      24 r-x-- ld-linux-x86-64.so.2
00007ffff7ff1000      40      40       0 r---- ld-linux-x86-64.so.2
00007ffff7ffb000       8       8       8 r---- ld-linux-x86-64.so.2
00007ffff7ffd000       8       8       8 rw--- ld-linux-x86-64.so.2
00007ffffffdd000     136      12      12 rw---   [ stack ]
ffffffffff600000       4       0       0 --x--   [ anon ]
---------------- ------- ------- ------- 
total kB            2560    1272     116
```

可以看到:

- 栈区地址区间 `[0x7fff'fffd'd000, 0x7fff'ffff'ffff]`
- 堆区地址空间 `[0x5555'5555'4000, 0x5555'5555'9fff]`

3. **简要说明应用程序与操作系统的异同.**

> 应用程序与操作系统相同的部分:
>
> 1. 都是程序, 都由代码及对应的数据构成
> 2. 都要遵循对应 ISA 平台的规范, 如指令规范、调用约定等.
>
> 不同的部分:
>
> - 角色与功能不同：应用程序负责具体的业务，操作系统负责管理应用和底层硬件、提供抽象、屏蔽底层差异
> - 具体规范不同：应用程序的运行环境、可执行的指令、可访问的外设等相比操作系统有更多限制

4. **请基于QEMU模拟RISC—V的执行过程和QEMU源代码，说明RISC-V硬件加电后的几条指令在哪里？完成了哪些功能？**

使用 QEMU + GDB 调试启动代码, 反汇编 [0x1000, 0x1020] 之间的指令:

```
(gdb) display /20i $pc
1: x/20i $pc
=> 0x1000:      auipc   t0,0x0
   0x1004:      addi    a2,t0,40
   0x1008:      csrr    a0,mhartid
   0x100c:      ld      a1,32(t0)
   0x1010:      ld      t0,24(t0)
   0x1014:      jr      t0
   ... (下面不是指令了)
```

查看 QEMU 对应源码如下:

```c
/* in hw/riscv/boot.c, line 397 */
void riscv_setup_rom_reset_vec(...)
{
    ...
        /* reset vector */
    uint32_t reset_vec[10] = {
        0x00000297,                  /* 1:  auipc  t0, %pcrel_hi(fw_dyn) */
        0x02828613,                  /*     addi   a2, t0, %pcrel_lo(1b) */
        0xf1402573,                  /*     csrr   a0, mhartid  */
        0,
        0,
        0x00028067,                  /*     jr     t0 */
        start_addr,                  /* start: .dword */
        start_addr_hi32,
        fdt_load_addr,               /* fdt_laddr: .dword */
        fdt_load_addr_hi32,
                                     /* fw_dyn: */
    };
    if (riscv_is_32bit(harts)) {
        reset_vec[3] = 0x0202a583;   /*     lw     a1, 32(t0) */
        reset_vec[4] = 0x0182a283;   /*     lw     t0, 24(t0) */
    } else {
        reset_vec[3] = 0x0202b583;   /*     ld     a1, 32(t0) */
        reset_vec[4] = 0x0182b283;   /*     ld     t0, 24(t0) */
    }

    if (!harts->harts[0].cfg.ext_zicsr) {
        /*
         * The Zicsr extension has been disabled, so let's ensure we don't
         * run the CSR instruction. Let's fill the address with a non
         * compressed nop.
         */
        reset_vec[2] = 0x00000013;   /*     addi   x0, x0, 0 */
    }
    ...
}
```

可以看出来, QEMU 在启动时先读取 HART-ID 到 a0, 然后跳转到 start_addr —— 也就是 SBI 的启动地址.

5. **RISC-V中的SBI的含义和功能是啥？**

含义: System Binary Interface, RISC-V 监管模式程序 (也就是操作系统) 在不同硬件下的兼容接口.

功能:
- 屏蔽具体的硬件差异, 实现监管模式程序在不同 RISC-V 实现之间的可移植性.
- 提供一组功能, 包括与硬件交互、管理虚拟内存、处理中断和异常等.

6. **为了让应用程序能在计算机上执行，操作系统与编译器之间需要达成哪些协议？**

- 操作系统向编译器提供程序库和 ABI, 以保证编译器的运行.
- 编译器向操作系统提供段位置、符号表、依赖等信息，保证操作系统能正确加载并运行编译产物。

7. **请简要说明从QEMU模拟的RISC-V计算机加电开始运行到执行应用程序的第一条指令这个阶段的执行过程。**

> 1. QEMU 启动, 通过启动代码加载 SBI 程序.
> 2. SBI 程序初始化硬件、加载内核，通过 mret 进入 S 模式并跳转到内核起始地址.
> 3. 内核初始化内存管理、进程管理、硬件驱动等内容, 加载并执行应用程序.

8. **为何应用程序员编写应用时不需要建立栈空间和指定地址空间？**

因为指定符号地址的操作由编译器、链接器和操作系统辅助完成了.

- 编译器(和静态链接器)在生成可执行文件时可以自动计算不同符号之间的地址, 通过修改程序中预留的点位指定可执行文件内部的地址关系. 动态链接的模块则会通过地址无关代码和 PLT 表等形式让程序可以装载在任意需要的位置, 从而减少地址冲突问题。
- 操作系统在装载应用程序时会自动建立栈空间、堆空间、映射空间等地址空间, 这些地址对操作系统来说都是虚拟地址. 操作系统通过硬件(如MMU)或软件映射的方式把不同应用程序的虚拟地址映射到不同的物理地址上, 这种地址空间隔离的做法可以有效减少地址冲突问题。

9. **现代的很多编译器生成的代码，默认情况下不再严格保存/恢复栈帧指针。在这个情况下，我们只要编译器提供足够的信息，也可以完成对调用栈的恢复。...**

> 由调试信息可知栈帧大小为 16 字节, 对于这些不保存帧指针的函数活动, 每次加 16 字节即可回到上一次函数活动。


