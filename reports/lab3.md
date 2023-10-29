# chapter5

## 一、功能

对于spawn调用，新建一个进程但维护它与调用者的父子关系。
对于stride算法，于TCB中建立用于维护信息的stride和pass字段，并修改任务调度的fetch方法，使其返回步长最小的进程。




## 二、问答题

### 1. 实际情况是轮到 p1 执行吗？为什么？

并不是p1执行，p2.stride = 250，加10后溢出为了一个更小的数，因此仍然是p2执行。


### 2. 为什么？尝试简单说明（不要求严格证明）。

假设当前进程中pass最长为的为``p_max``，其pass为``MAX_PASS``，那么当且仅当其他所有进程的步长都大于``p_max``的步长，才会调度到``p_max``并将步长加上``MAX_PASS``，此时其余进程的步长位于``p_max``的新旧步长中间，可得它们相差不会超过``MAX_PASS``，可得``STRIDE_MAX – STRIDE_MIN <= MAX_PASS``。当``优先级 >= 2``时，``MAX_PASS``不会超过``BigStride / 2``，即``STRIDE_MAX – STRIDE_MIN <= BigStride / 2``。


### 3. 已知以上结论，考虑溢出的情况下，可以为 Stride 设计特别的比较器，让 BinaryHeap<Stride> 的 pop 方法能返回真正最小的 Stride。补全下列代码中的 partial_cmp 函数，假设两个 Stride 永远不会相等。

```rust
use core::cmp::Ordering;

struct Stride(u64);

impl PartialOrd for Stride {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.0 >= other.0{
            if self.0 - other.0 <= u64::MAX / 2{
                Some(Greater)
            }else{
                Some(Less)
            }
        }
        else{
            if other.0 - self.0 <= u64::MAX / 2{
                Some(Less)
            }else{
                Some(Greater)
            }
        }
    }
}

impl PartialEq for Stride {
    fn eq(&self, other: &Self) -> bool {
        false
    }
}
```







## 三、感受与建议

### 1. 代码框架

hummm我不清楚是否是想以“合并上一章内容”的方式迫使同学们rtfsc，
但是当你大概有了做本章的思路，但先得把上一章已经写过的东西再倒腾一遍……感觉不是很“连续”？








# 荣誉准则

1. 在完成本次实验的过程（含此前学习的过程）中，我曾分别与**以下各位**就（与本次实验相关的）以下方面做过交流，还在代码中对应的位置以注释形式记录了具体的交流对象及内容：

> NULL

2. 此外，我也参考了**以下资料**，还在代码中对应的位置以注释形式记录了具体的参考来源及内容：

> https://course.rs/advance/smart-pointer/rc-arc.html

3. 我独立完成了本次实验除以上方面之外的所有工作，包括代码与文档。我清楚地知道，从以上方面获得的信息在一定程度上降低了实验难度，可能会影响起评分。

4. 我从未使用过他人的代码，不管是原封不动地复制，还是经过了某些等价转换。我未曾也不会向他人（含此后各届同学）复制或公开我的实验代码，我有义务妥善保管好它们。我提交至本实验的评测系统的代码，均无意于破坏或妨碍任何计算机系统的正常运转。我清楚地知道，以上情况均为本课程纪律所禁止，若违反，对应的实验成绩将按“-100”分计。

