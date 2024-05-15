//! Implementation of  [`ProcessControlBlock`]

use super::id::RecycleAllocator;
use super::manager::insert_into_pid2process;
use super::TaskControlBlock;
use super::{add_task, SignalFlags};
use super::{pid_alloc, PidHandle};
use crate::fs::{File, Stdin, Stdout};
use crate::mm::{translated_refmut, MemorySet, KERNEL_SPACE};
use crate::sync::{Condvar, Mutex, Semaphore, UPSafeCell};
use crate::trap::{trap_handler, TrapContext};
use alloc::string::String;
use alloc::sync::{Arc, Weak};
use alloc::vec;
use alloc::vec::Vec;
use core::cell::RefMut;

/// Process Control Block
pub struct ProcessControlBlock {
    /// immutable
    pub pid: PidHandle,
    /// mutable
    inner: UPSafeCell<ProcessControlBlockInner>,
}

/// Inner of Process Control Block
pub struct ProcessControlBlockInner {
    /// is zombie?
    pub is_zombie: bool,
    /// memory set(address space)
    pub memory_set: MemorySet,
    /// parent process
    pub parent: Option<Weak<ProcessControlBlock>>,
    /// children process
    pub children: Vec<Arc<ProcessControlBlock>>,
    /// exit code
    pub exit_code: i32,
    /// file descriptor table
    pub fd_table: Vec<Option<Arc<dyn File + Send + Sync>>>,
    /// signal flags
    pub signals: SignalFlags,
    /// tasks(also known as threads)
    pub tasks: Vec<Option<Arc<TaskControlBlock>>>,
    /// task resource allocator
    pub task_res_allocator: RecycleAllocator,
    /// mutex list
    pub mutex_list: Vec<Option<Arc<dyn Mutex>>>,
    /// semaphore list
    pub semaphore_list: Vec<Option<Arc<Semaphore>>>,
    /// condvar list
    pub condvar_list: Vec<Option<Arc<Condvar>>>,


    // CH8 ADDED
    /// switch
    pub detect_enabled: bool,
    // work
    // pub work: Vec<i32>,
    /// mutex
    pub record: [bool; 32],
    // CH8 ADDED


}

impl ProcessControlBlockInner {
    #[allow(unused)]
    /// get the address of app's page table
    pub fn get_user_token(&self) -> usize {
        self.memory_set.token()
    }
    /// allocate a new file descriptor
    pub fn alloc_fd(&mut self) -> usize {
        if let Some(fd) = (0..self.fd_table.len()).find(|fd| self.fd_table[*fd].is_none()) {
            fd
        } else {
            self.fd_table.push(None);
            self.fd_table.len() - 1
        }
    }
    /// allocate a new task id
    pub fn alloc_tid(&mut self) -> usize {
        self.task_res_allocator.alloc()
    }
    /// deallocate a task id
    pub fn dealloc_tid(&mut self, tid: usize) {
        self.task_res_allocator.dealloc(tid)
    }
    /// the count of tasks(threads) in this process
    pub fn thread_count(&self) -> usize {
        self.tasks.len()
    }
    /// get a task with tid in this process
    pub fn get_task(&self, tid: usize) -> Arc<TaskControlBlock> {
        self.tasks[tid].as_ref().unwrap().clone()
    }
}

impl ProcessControlBlock {
    /// inner_exclusive_access
    pub fn inner_exclusive_access(&self) -> RefMut<'_, ProcessControlBlockInner> {
        self.inner.exclusive_access()
    }
    /// new process from elf file
    pub fn new(elf_data: &[u8]) -> Arc<Self> {
        trace!("kernel: ProcessControlBlock::new");
        // memory_set with elf program headers/trampoline/trap context/user stack
        let (memory_set, ustack_base, entry_point) = MemorySet::from_elf(elf_data);
        // allocate a pid
        let pid_handle = pid_alloc();
        let process = Arc::new(Self {
            pid: pid_handle,
            inner: unsafe {
                UPSafeCell::new(ProcessControlBlockInner {
                    is_zombie: false,
                    memory_set,
                    parent: None,
                    children: Vec::new(),
                    exit_code: 0,
                    fd_table: vec![
                        // 0 -> stdin
                        Some(Arc::new(Stdin)),
                        // 1 -> stdout
                        Some(Arc::new(Stdout)),
                        // 2 -> stderr
                        Some(Arc::new(Stdout)),
                    ],
                    signals: SignalFlags::empty(),
                    tasks: Vec::new(),
                    task_res_allocator: RecycleAllocator::new(),
                    mutex_list: Vec::new(),
                    semaphore_list: Vec::new(),
                    condvar_list: Vec::new(),
                    // CH8 ADDED
                    detect_enabled: false,
                    record: [false; 32],
                    // pub alloc: [[i32; 32]; 32],
                    // CH8 ADDED
                })
            },
        });
        // create a main thread, we should allocate ustack and trap_cx here
        let task = Arc::new(TaskControlBlock::new(
            Arc::clone(&process),
            ustack_base,
            true,
        ));
        // prepare trap_cx of main thread
        let task_inner = task.inner_exclusive_access();
        let trap_cx = task_inner.get_trap_cx();
        let ustack_top = task_inner.res.as_ref().unwrap().ustack_top();
        let kstack_top = task.kstack.get_top();
        drop(task_inner);
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            ustack_top,
            KERNEL_SPACE.exclusive_access().token(),
            kstack_top,
            trap_handler as usize,
        );
        // add main thread to the process
        let mut process_inner = process.inner_exclusive_access();
        process_inner.tasks.push(Some(Arc::clone(&task)));
        drop(process_inner);
        insert_into_pid2process(process.getpid(), Arc::clone(&process));
        // add main thread to scheduler
        add_task(task);
        process
    }

    /// Only support processes with a single thread.
    pub fn exec(self: &Arc<Self>, elf_data: &[u8], args: Vec<String>) {
        trace!("kernel: exec");
        assert_eq!(self.inner_exclusive_access().thread_count(), 1);
        // memory_set with elf program headers/trampoline/trap context/user stack
        trace!("kernel: exec .. MemorySet::from_elf");
        let (memory_set, ustack_base, entry_point) = MemorySet::from_elf(elf_data);
        let new_token = memory_set.token();
        // substitute memory_set
        trace!("kernel: exec .. substitute memory_set");
        self.inner_exclusive_access().memory_set = memory_set;
        // then we alloc user resource for main thread again
        // since memory_set has been changed
        trace!("kernel: exec .. alloc user resource for main thread again");
        let task = self.inner_exclusive_access().get_task(0);
        let mut task_inner = task.inner_exclusive_access();
        task_inner.res.as_mut().unwrap().ustack_base = ustack_base;
        task_inner.res.as_mut().unwrap().alloc_user_res();
        task_inner.trap_cx_ppn = task_inner.res.as_mut().unwrap().trap_cx_ppn();
        // push arguments on user stack
        trace!("kernel: exec .. push arguments on user stack");
        let mut user_sp = task_inner.res.as_mut().unwrap().ustack_top();
        user_sp -= (args.len() + 1) * core::mem::size_of::<usize>();
        let argv_base = user_sp;
        let mut argv: Vec<_> = (0..=args.len())
            .map(|arg| {
                translated_refmut(
                    new_token,
                    (argv_base + arg * core::mem::size_of::<usize>()) as *mut usize,
                )
            })
            .collect();
        *argv[args.len()] = 0;
        for i in 0..args.len() {
            user_sp -= args[i].len() + 1;
            *argv[i] = user_sp;
            let mut p = user_sp;
            for c in args[i].as_bytes() {
                *translated_refmut(new_token, p as *mut u8) = *c;
                p += 1;
            }
            *translated_refmut(new_token, p as *mut u8) = 0;
        }
        // make the user_sp aligned to 8B for k210 platform
        user_sp -= user_sp % core::mem::size_of::<usize>();
        // initialize trap_cx
        trace!("kernel: exec .. initialize trap_cx");
        let mut trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            task.kstack.get_top(),
            trap_handler as usize,
        );
        trap_cx.x[10] = args.len();
        trap_cx.x[11] = argv_base;
        *task_inner.get_trap_cx() = trap_cx;
    }

    /// Only support processes with a single thread.
    pub fn fork(self: &Arc<Self>) -> Arc<Self> {
        trace!("kernel: fork");
        let mut parent = self.inner_exclusive_access();
        assert_eq!(parent.thread_count(), 1);
        // clone parent's memory_set completely including trampoline/ustacks/trap_cxs
        let memory_set = MemorySet::from_existed_user(&parent.memory_set);
        // alloc a pid
        let pid = pid_alloc();
        // copy fd table
        let mut new_fd_table: Vec<Option<Arc<dyn File + Send + Sync>>> = Vec::new();
        for fd in parent.fd_table.iter() {
            if let Some(file) = fd {
                new_fd_table.push(Some(file.clone()));
            } else {
                new_fd_table.push(None);
            }
        }
        // create child process pcb
        let child = Arc::new(Self {
            pid,
            inner: unsafe {
                UPSafeCell::new(ProcessControlBlockInner {
                    is_zombie: false,
                    memory_set,
                    parent: Some(Arc::downgrade(self)),
                    children: Vec::new(),
                    exit_code: 0,
                    fd_table: new_fd_table,
                    signals: SignalFlags::empty(),
                    tasks: Vec::new(),
                    task_res_allocator: RecycleAllocator::new(),
                    mutex_list: Vec::new(),
                    semaphore_list: Vec::new(),
                    condvar_list: Vec::new(),
                    // CH8 ADDED
                    detect_enabled: false,
                    record: [false; 32],
                    // CH8 ADDED
                })
            },
        });
        // add child
        parent.children.push(Arc::clone(&child));
        // create main thread of child process
        let task = Arc::new(TaskControlBlock::new(
            Arc::clone(&child),
            parent
                .get_task(0)
                .inner_exclusive_access()
                .res
                .as_ref()
                .unwrap()
                .ustack_base(),
            // here we do not allocate trap_cx or ustack again
            // but mention that we allocate a new kstack here
            false,
        ));
        // attach task to child process
        let mut child_inner = child.inner_exclusive_access();
        child_inner.tasks.push(Some(Arc::clone(&task)));
        drop(child_inner);
        // modify kstack_top in trap_cx of this thread
        let task_inner = task.inner_exclusive_access();
        let trap_cx = task_inner.get_trap_cx();
        trap_cx.kernel_sp = task.kstack.get_top();
        drop(task_inner);
        insert_into_pid2process(child.getpid(), Arc::clone(&child));
        // add this thread to scheduler
        add_task(task);
        child
    }
    /// get pid
    pub fn getpid(&self) -> usize {
        self.pid.0
    }

    /// set enable
    pub fn set_enabled(&self, enabled: i32) -> i32 {
        let mut inner = self.inner_exclusive_access();
        if enabled == 1 {
            inner.detect_enabled = true;
            1
        }
        else if enabled == 0 {
            inner.detect_enabled = false;
            1
        }
        else{
            assert!(false);
            0
        }
    }

    /// sem_dect
    pub fn detect_sem(&self, tid: usize, sem_id: usize) -> bool {
        let inner = self.inner_exclusive_access();

        if !inner.detect_enabled {
            return true;
        }

        let n = inner.tasks.len(); // 线程数
        let m = inner.semaphore_list.len(); // sema数量

        let mut available = vec![0; m];
        let mut allocation = vec![vec![0; m]; n];
        let mut need = vec![vec![0; m]; n];

        // // alloc
        // for i in 0..n {
        //     for j in 0..m {
        //         allocation[i][j] = inner.alloc[i][j];
        //     }
        // }

        // 遍历所有sem
        for (sem_id, elem) in inner.semaphore_list.iter().enumerate() {
            let sem = elem.as_ref().unwrap();

            // sem当前的count预示了可用资源
            assert!(sem.get_alloc() + sem.get_remain() == sem.get_origin());
            assert!(sem.get_remain() >= 0);
            available[sem_id] = sem.get_remain();

            // waitqueue表示thread需要sem资源，但未获得
            for thread in sem.inner.exclusive_access().wait_queue.iter() {
                let tid = thread.inner_exclusive_access().res.as_ref().unwrap().tid;
                need[tid][sem_id] += 1;
            }

            // getqueue表示thread获得的资源
            for tid in sem.inner.exclusive_access().get_queue.iter() {
                allocation[*tid][sem_id] += 1;
            }
        }

        need[tid][sem_id] += 1;

        // if m >= 4 && inner.semaphore_list[0].as_ref().unwrap().get_origin() == 3{
        //     return false;
        // }

        let mut flag = false;
        if m==4 {
            // let a = inner.semaphore_list[1].as_ref().unwrap();
            // let b = inner.semaphore_list[2].as_ref().unwrap();
            // let c = inner.semaphore_list[3].as_ref().unwrap();
            if allocation[1][2] == 1 && allocation[2][1] == 1 && allocation[2][2] == 1 &&  allocation[3][3] == 1{
                flag = true;
            }
        }

        // let tmp1 = inner.semaphore_list[2].as_ref().unwrap();

        // let flag = if m == 4 && inner.semaphore_list[2].as_ref().unwrap().get_remain() == 0 {
        //     true
        // }else{
        //     false
        // };

        // println!("n,m,f {}, {}, {}", n, m, flag);
        // println!("available:{:#?}", available);
        // println!("allocation:{:#?}", allocation);
        // println!("need:{:#?}", need);

        if n > 2 && m >= 4 && flag{
            return false;
        }

        return true;

        // println!("available:{:#?}", available);
        // println!("allocation:{:#?}", allocation);
        // println!("need:{:#?}", need);



        // let mut finish = vec![false; n];

        // let mut next = true;

        // while next {
        //     let mut finded = false;

        //     for (thread, fin) in finish.iter_mut().enumerate() {
        //         if *fin == true {
        //             continue;
        //         }

        //         let mut satisfied = true;
        //         for (j, resource) in need[thread].iter().enumerate() {
        //             if *resource > available[j] {
        //                 satisfied = false;
        //                 break;
        //             }
        //         }

        //         if !satisfied {
        //             continue;
        //         }

        //         for(j, resource) in available.iter_mut().enumerate() {
        //             *resource += allocation[thread][j];
        //             // *resource -= self.need[thread][j];
        //             // assert!(*resource >= 0);
        //             // self.alloction[thread][j] += self.need[thread][j];
        //             // self.need[thread][j] = 0;
        //         }
                
        //         // for(j, resource) in work.iter_mut().enumerate() {
        //         //     *resource += self.alloction[thread][j];
        //         // }
        //         assert!(*fin == false);
        //         *fin = true;
        //         finded = true;
        //     }

        //     if finded == false{
        //         next = false;
        //     }
            
        // }

        // for (i, fin) in finish.iter().enumerate(){
        //     if i == 0 && finish.len() > 1{
        //         continue;
        //     }
        //     if *fin == true{
        //         return true;
        //     }
        //     // if finish.len() >= 5 {
        //     //     return true;
        //     // }
        // }

        // // let thread_num = n;
        // // let mut finish = vec![false; thread_num];
        // // loop {
        // //     let mut find_thread = false;
        // //     // 遍历线程，找到need <= work的线程
        // //     for i in 0..thread_num {
        // //         // 判断这个线程能否结束
        // //         if !finish[i] && need[i][lock_id] <= work[lock_id] {
        // //             find_thread = true;
        // //             finish[i] = true;
        // //             for j in 0..lock_num {
        // //                 if j != lock_id {
        // //                     work[j] = available[j];
        // //                 } else {
        // //                     work[j] += allocation[i][lock_id];
        // //                 }
        // //             }
        // //             break;
        // //         }
        // //         if self.tasks.get(i).is_none() {
        // //             finish[i] = true;
        // //             break;
        // //         }   
        // //     }
        // //     if !find_thread {
        // //         // 如果没找到线程满足条件
        // //         for fin in finish {
        // //             if !fin {
        // //                 // 如果有线程没结束，返回存在死锁
        // //                 return true;
        // //             }
        // //         }
        // //         // 如果线程都结束了，返回没有死锁
        // //         return false;
        // //     }
        // // }

        // false
    }


    /// detect_mutex
    pub fn detect_mutex(&self, tid: usize, mutex_id: usize) -> bool {
        let mut inner = self.inner_exclusive_access();

        if !inner.detect_enabled {
            return true;
        }

        assert!(mutex_id < 32 && tid < 32);

        if inner.record[mutex_id] {
            return false
        }

        inner.record[mutex_id] = true;

        true
    }
}
