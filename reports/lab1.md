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

### 1.蜜汁文档

当发现有《Tutorial-Guide》和《Tutorial-Book》时，第一反应还是困惑：我应该看哪个？还是都看？
（做过一些课程的lab，但是有两套手册的我还是第一次见
尤其是发现它们之间并非严格的包含关系时————《Tutorial-Book》中的操作并不一定在Lab中成立

例如，https://rcore-os.cn/rCore-Tutorial-Book-v3/chapter2/0intro.html 中写到通过
```bash
git clone https://github.com/rcore-os/rCore-Tutorial-v3.git
```
获取框架代码，然而Lab的Classroom仓库并不与其一致，此时若``cd os && make run``是必然报错的：Classroom仓库并没有``user``
事实上这部分内容在《Tutorial-Guide》https://learningos.cn/rCore-Tutorial-Guide-2023A/chapter2/0intro.html 中明确表示了要手动clone。

《Tutorial-Book》有更详细的内容，是大多数新手会选择详看的部分；但你却不太能照着《Tutorial-Book》实操，否则会遇到上面的问题————除非专门准备一套《Tutorial-Book》的仓库。而要完成Lab时，似乎应该紧盯《Tutorial-Guide》。

当然，由于我听课不多，不清楚课堂上是否给出了如何使用两本手册的指导，比如理解知识以《Tutorial-Book》为主，实践要看《Tutorial-Guide》之类的；在没有其他信息的情况下，如何正确使用手册还是让新手的我感到有些困惑。

### 2.测试框架

我怀疑ci-user这套测评系统是否太有“侵略性”了？
首先，默认的test目标会直接卸载重装一波rust的环境：
```makefile
env:
	rustup uninstall nightly && rustup install nightly
	(rustup target list | grep "riscv64gc-unknown-none-elf (installed)") || rustup target add riscv64gc-unknown-none-elf
	cargo install cargo-binutils
	rustup component add rust-src
	rustup component add llvm-tools-preview

test: env randomize
```
我想没有必要每次测试都重整一遍吧……

其次它会直接覆盖Makefile等文件，干扰原先的使用。

所以难道说它并不是给用户使用的？如果是这样那为什么要将其使用方法要放在仓库的readme里呢
（rustling的classroom仓库README似乎也有类似问题，即尽管readme里写了使用方法但用户其实不可以按照那个来，需要把官方手册提到的仓库换成自己的classroom仓库。可能除非classroom能针对不同用户生成不同内容，不然这个也不太好搞）








# 荣誉准则

1. 在完成本次实验的过程（含此前学习的过程）中，我曾分别与**以下各位**就（与本次实验相关的）以下方面做过交流，还在代码中对应的位置以注释形式记录了具体的交流对象及内容：

> NULL

2. 此外，我也参考了**以下资料**，还在代码中对应的位置以注释形式记录了具体的参考来源及内容：

> 南京大学PA2021：https://nju-projectn.github.io/ics-pa-gitbook/ics2021/3.2.html

3. 我独立完成了本次实验除以上方面之外的所有工作，包括代码与文档。我清楚地知道，从以上方面获得的信息在一定程度上降低了实验难度，可能会影响起评分。

4. 我从未使用过他人的代码，不管是原封不动地复制，还是经过了某些等价转换。我未曾也不会向他人（含此后各届同学）复制或公开我的实验代码，我有义务妥善保管好它们。我提交至本实验的评测系统的代码，均无意于破坏或妨碍任何计算机系统的正常运转。我清楚地知道，以上情况均为本课程纪律所禁止，若违反，对应的实验成绩将按“-100”分计。

