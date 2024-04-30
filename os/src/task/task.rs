//! Types related to task management

use super::TaskContext;

/// The task control block (TCB) of a task.
#[derive(Copy, Clone)]
pub struct TaskControlBlock {
    /// The task status in it's lifecycle
    pub task_status: TaskStatus,
    /// The task context
    pub task_cx: TaskContext,

    //CH3 ADDED
    /// ms
    pub start_time: usize,
    /// count
    pub syscall_times: [u32; crate::config::MAX_SYSCALL_NUM], //TODO: too much space
}

/// The status of a task
#[derive(Copy, Clone, PartialEq)]
pub enum TaskStatus {
    /// uninitialized
    UnInit,
    /// ready to run
    Ready,
    /// running
    Running,
    /// exited
    Exited,
}

// const MAX_SYSCALL_NUM: usize = 500;

// #[derive(Copy, Clone)]
// pub struct TaskInfo {
//     pub status: TaskStatus,
//     pub syscall_times: [u32; MAX_SYSCALL_NUM],
//     pub time: usize,
// }

// impl TaskInfo {
//     pub fn new() -> Self {
//         TaskInfo {
//             status: TaskStatus::UnInit,
//             syscall_times: [0; MAX_SYSCALL_NUM],
//             time: 0,
//         }
//     }
// }
