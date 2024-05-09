//! Task management implementation
//!
//! Everything about task management, like starting and switching tasks is
//! implemented here.
//!
//! A single global instance of [`TaskManager`] called `TASK_MANAGER` controls
//! all the tasks in the whole operating system.
//!
//! A single global instance of [`Processor`] called `PROCESSOR` monitors running
//! task(s) for each core.
//!
//! A single global instance of `PID_ALLOCATOR` allocates pid for user apps.
//!
//! Be careful when you see `__switch` ASM function in `switch.S`. Control flow around this function
//! might not be what you expect.
mod context;
mod id;
mod manager;
mod processor;
mod switch;
#[allow(clippy::module_inception)]
#[allow(rustdoc::private_intra_doc_links)]
mod task;

use crate::fs::{open_file, OpenFlags};
use alloc::sync::Arc;
pub use context::TaskContext;
use lazy_static::*;
pub use manager::{fetch_task, TaskManager};
use switch::__switch;
pub use task::{TaskControlBlock, TaskStatus};

pub use id::{kstack_alloc, pid_alloc, KernelStack, PidHandle};
pub use manager::add_task;
pub use processor::{
    current_task, current_trap_cx, current_user_token, run_tasks, schedule, take_current_task,
    Processor,
};
/// Suspend the current 'Running' task and run the next task in task list.
pub fn suspend_current_and_run_next() {
    // There must be an application running.
    let task = take_current_task().unwrap();

    // ---- access current TCB exclusively
    let mut task_inner = task.inner_exclusive_access();
    let task_cx_ptr = &mut task_inner.task_cx as *mut TaskContext;
    // Change status to Ready
    task_inner.task_status = TaskStatus::Ready;
    drop(task_inner);
    // ---- release current PCB

    // push back to ready queue.
    add_task(task);
    // jump to scheduling cycle
    schedule(task_cx_ptr);
}

/// pid of usertests app in make run TEST=1
pub const IDLE_PID: usize = 0;

/// Exit the current 'Running' task and run the next task in task list.
pub fn exit_current_and_run_next(exit_code: i32) {
    // take from Processor
    let task = take_current_task().unwrap();

    let pid = task.getpid();
    if pid == IDLE_PID {
        println!(
            "[kernel] Idle process exit with exit_code {} ...",
            exit_code
        );
        panic!("All applications completed!");
    }

    // **** access current TCB exclusively
    let mut inner = task.inner_exclusive_access();
    // Change status to Zombie
    inner.task_status = TaskStatus::Zombie;
    // Record exit code
    inner.exit_code = exit_code;
    // do not move to its parent but under initproc

    // ++++++ access initproc TCB exclusively
    {
        let mut initproc_inner = INITPROC.inner_exclusive_access();
        for child in inner.children.iter() {
            child.inner_exclusive_access().parent = Some(Arc::downgrade(&INITPROC));
            initproc_inner.children.push(child.clone());
        }
    }
    // ++++++ release parent PCB

    inner.children.clear();
    // deallocate user space
    inner.memory_set.recycle_data_pages();
    // drop file descriptors
    inner.fd_table.clear();
    drop(inner);
    // **** release current PCB
    // drop task manually to maintain rc correctly
    drop(task);
    // we do not have to save task context
    let mut _unused = TaskContext::zero_init();
    schedule(&mut _unused as *mut _);
}

lazy_static! {
    /// Creation of initial process
    ///
    /// the name "initproc" may be changed to any other app name like "usertests",
    /// but we have user_shell, so we don't need to change it.
    pub static ref INITPROC: Arc<TaskControlBlock> = Arc::new({
        let inode = open_file("ch6b_initproc", OpenFlags::RDONLY).unwrap();
        let v = inode.read_all();
        TaskControlBlock::new(v.as_slice())
    });
}

///Add init process to the manager
pub fn add_initproc() {
    add_task(INITPROC.clone());
}

//CH3 ADDED: task_info
/// Update syscall count.
pub fn update_info_syscall(id: usize){
    let task = take_current_task().unwrap();
    let mut inner = task.inner_exclusive_access();

    inner.syscall_times[id] += 1;

    drop(inner);
    //TODO: 太丑陋了哥
    processor::return_current_task(task);
    // info!("[kernel] task[{}] calling syscall[{}]!", current, id);
}
    
/// Set start time.
pub fn update_info_starttime(){
    let task = take_current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    
    if inner.start_time == 0 {
        inner.start_time = crate::timer::get_time_us()/1000;
        // println!("[kernel] task[{}] first run and set timestamp of {}!", current, inner.tasks[current].start_time);
    }

    drop(inner);
    //TODO: 太丑陋了哥
    processor::return_current_task(task);
}
    
/// get syscall count.
pub fn get_info_syscall() -> [u32; crate::config::MAX_SYSCALL_NUM]{
    //TODO: 是否应该使用应用减少拷贝开销?
    let task = take_current_task().unwrap();
    let inner = task.inner_exclusive_access();

    let res = inner.syscall_times;

    drop(inner);
    //TODO: 太丑陋了哥
    processor::return_current_task(task);
    res
}
    
/// get syscall count.
pub fn get_info_starttime() -> usize{
    let task = take_current_task().unwrap();
    let inner = task.inner_exclusive_access();

    let res = inner.start_time;

    drop(inner);
    //TODO: 太丑陋了哥
    processor::return_current_task(task);
    res
}
//CH3 ADDED: task_info
    
//CH4 ADDED
/// _start should be aligned, success return 0 else -1
pub fn current_add_map(_start: usize, _end: usize, _perm: crate::mm::MapPermission) -> isize{
    let task = take_current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    
    let res = & mut inner.memory_set;
    
    if (*res).check_overlap(_start.into(), _end.into()){
        //TODO: 太丑陋了哥
        drop(inner);
        processor::return_current_task(task);
        println!("[current_add_map : [{:#x}, {:#x}) ] has overlap!", _start, _end);
        return -1;
    }
    
    (*res).insert_framed_area(_start.into(), _end.into(), _perm);
    drop(inner);
    //TODO: 太丑陋了哥
    processor::return_current_task(task);
    0
}
    
/// remove WHOLE AreaSegment which contain the range. _start should be aligned, success return 0 else -1
pub fn current_remove_map(_start: usize, _end: usize) -> isize{
    let task = take_current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    
    let res = & mut inner.memory_set;
    
    if (*res).remove_map(_start.into(), _end.into()) {
        //TODO: 太丑陋了哥
        drop(inner);
        processor::return_current_task(task);
        return 0;
    }
    drop(inner);
    //TODO: 太丑陋了哥
    processor::return_current_task(task);
    -1
}
//CH4 ADDED

//CH5 ADDED: stride
///
pub fn set_priority(_prio: isize) -> isize{
    let task = take_current_task().unwrap();

    let pass_val = self::task::BIG_STRIDE / _prio as usize;

    let mut pass = task.pass.exclusive_access();

    *pass = pass_val;

    drop(pass);

    //TODO: 太丑陋了哥
    processor::return_current_task(task);
    _prio
}

// ///
// pub fn update_stride(){
//     let task = take_current_task().unwrap();

//     let prio = task.priority.exclusive_access();
//     let mut stride = task.stride.exclusive_access();

//     *stride +=  self::task::BIG_STRIDE / *prio;

//     drop(prio);
//     drop(stride);

//     //TODO: 太丑陋了哥
//     processor::return_current_task(task);
// }
//CH5 ADDED: stride
