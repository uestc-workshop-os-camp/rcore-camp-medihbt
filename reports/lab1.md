# RCore-OS Lab1 - LibOS

## 实现的功能

基本进入了用户态；实现了 `sys_get_time` `sys_task_info` 系统调用.

## 问答题

1. 正确进入 U 态后，程序的特征还应有：使用 S 态特权指令，访问 S 态寄存器后会报错。 请同学们描述程序出错行为，同时注意注明你使用的 sbi 及其版本。

上述 3 个测例会报下述错误:

```log
[kernel] PageFault in application, bad addr = 0x0, bad instruction = 0x804003ac, kernel killed it.
[kernel] IllegalInstruction in application, kernel killed it.
[kernel] IllegalInstruction in application, kernel killed it.
```

RustSBI 版本: `RustSBI-QEMU Version 0.2.0-alpha.2`

2. 深入理解 trap.S 中两个函数 `__alltraps` 和 `__restore` 的作用，并回答如下问题:

1) L40：刚进入 `__restore` 时，a0 代表了什么值。请指出 `__restore` 的两种使用情景。

a0 此时代表指向 trap_ctx 的指针.

`__restore` 的两种使用场景分别是：(1) 执行第一个应用程序; (2) 从 trap 返回用户态.

2) L43-L48：这几行汇编代码特殊处理了哪些寄存器？这些寄存器的的值对于进入用户态有何意义？请分别解释。

`sstatus` 寄存器: 用户保存的状态寄存器.
`sepc` 寄存器: 用户态返回地址
`sscratch` 寄存器: 用户态的 SP 指针.

3) L50-L56：为何跳过了 x2 和 x4？

x2 是当前 trap 态的 sp 指针, x4 是 tp 指针——这对没有线程的 rCore OS 来说没用.

4) L60：该指令之后，sp 和 sscratch 中的值分别有什么意义？

sp: 用户态 sp 指针
sscratch: trap handler 的 sp 指针

5) __restore：中发生状态切换在哪一条指令？为何该指令执行之后会进入用户态？

发生在 L61 sret 指令. 执行该指令后修改特权级为 U, 交换 pc 与 sepc, 返回用户态.

6) L13：该指令之后，sp 和 sscratch 中的值分别有什么意义？

sp: trap handler 的 sp 指针
sscratch: 用户态保存的 sp 指针

7) 从 U 态进入 S 态是哪一条指令发生的？

ecall 指令

## 荣誉准则



1. 在完成本次实验的过程（含此前学习的过程）中，我曾分别与 以下各位 就（与本次实验相关的）以下方面做过交流，还在代码中对应的位置以注释形式记录了具体的交流对象及内容：

> 文心一言: 学习 RISC-V 相关知识、查阅相关资料

2. 此外，我也参考了 以下资料 ，还在代码中对应的位置以注释形式记录了具体的参考来源及内容：

> RvBook

3. 我独立完成了本次实验除以上方面之外的所有工作，包括代码与文档。 我清楚地知道，从以上方面获得的信息在一定程度上降低了实验难度，可能会影响起评分。

4. 我从未使用过他人的代码，不管是原封不动地复制，还是经过了某些等价转换。 我未曾也不会向他人（含此后各届同学）复制或公开我的实验代码，我有义务妥善保管好它们。 我提交至本实验的评测系统的代码，均无意于破坏或妨碍任何计算机系统的正常运转。 我清楚地知道，以上情况均为本课程纪律所禁止，若违反，对应的实验成绩将按“-100”分计。