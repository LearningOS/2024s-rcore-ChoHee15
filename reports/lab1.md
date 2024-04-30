# chapter3

## 一、功能

往``TaskControlBlock``里存了初次运行时间和系统调用次数的统计数组。
在调度时查看是否为首次运行（初次运行时间是否为0），若是则获取当前时间存入。程序运行时间即``当前时间-初次运行时间``。
在``trap_handler``获取系统调用时间时，增加当前程序的系统调用统计计数。

//TODO
当系统调用数量较多时，统计数组的使用率似乎不会很高。很少有程序能把syscall用个一大半。
数量越多，拷贝的负担就越大。
应该有更好的方式？





## 二、问答题

### 1.错误用例

三个程序分别尝试在0x0非法地址写入、使用sret指令、写csr寄存器；分别报相应错误
```bash
[kernel] PageFault in application, bad addr = 0x0, bad instruction = 0x804003c4, kernel killed it.
[kernel] IllegalInstruction in application, kernel killed it.
[kernel] IllegalInstruction in application, kernel killed it.
```
sbi版本：``RustSBI version 0.3.0-alpha.2, adapting to RISC-V SBI v1.0.0``

### 2. __alltraps 和 __restore

1. 刚进入 __restore 时，a0 代表了TrapContext的地址。第一种情况是，对于大部分系统调用，__restore从本进程的TrapContext恢复。第二种情况是发生任务切换，__restore从其他进程的TrapContext恢复（除非又调度到自己）。

2. 这几行汇编代码分别处理了sstatus、sepc和sscratch寄存器。sstatus存放了各种标志信息如特权级、是否允许中断等；sepc标示了从Trap恢复后要回到哪执行；sscratch此时会存放用户栈的地址，之后会将其和sp互换以回到正常的栈位置并存放内核栈地址。

3. x2/sp寄存器表示了栈的位置，在保存上下文完成后，它的值会交由``trap_handler``处理，当``trap_handler``返回后，它会回到恰当的位置，这一过程由``trap_handler``和编译器负责，因此不用保存；而x4/tp寄存器暂时不会使用到，因此不需要保存。

4. L60的``csrrw sp, sscratch, sp``后，sp为用户栈地址，而sscratch存放内核栈地址。

5. ``__restore``中，``sret``发生状态切换，它回设置sstatus的特权级状态，并跳转到sepc标示的指令。

6. L13的``csrrw sp, sscratch, sp``后，sp为内核栈地址，而sscratch存放用户栈地址。

7. 从 U 态进入 S 态是通过``ecall``指令完成的。





## 三、感受与建议

2023A参加过，classroom的说明更新了，that's gooooooood

测试框架也不会每次运行都卸载重装一波rust的环境了，that's pretty gooooooood

不过还是会改动Makefile和build.rs，如果这个能解决将是绝杀。




# 荣誉准则

1. 在完成本次实验的过程（含此前学习的过程）中，我曾分别与**以下各位**就（与本次实验相关的）以下方面做过交流，还在代码中对应的位置以注释形式记录了具体的交流对象及内容：

> NULL

2. 此外，我也参考了**以下资料**，还在代码中对应的位置以注释形式记录了具体的参考来源及内容：

> 南京大学PA2021：https://nju-projectn.github.io/ics-pa-gitbook/ics2021/3.2.html

3. 我独立完成了本次实验除以上方面之外的所有工作，包括代码与文档。我清楚地知道，从以上方面获得的信息在一定程度上降低了实验难度，可能会影响起评分。

4. 我从未使用过他人的代码，不管是原封不动地复制，还是经过了某些等价转换。我未曾也不会向他人（含此后各届同学）复制或公开我的实验代码，我有义务妥善保管好它们。我提交至本实验的评测系统的代码，均无意于破坏或妨碍任何计算机系统的正常运转。我清楚地知道，以上情况均为本课程纪律所禁止，若违反，对应的实验成绩将按“-100”分计。

