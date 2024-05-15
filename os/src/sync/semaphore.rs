//! Semaphore

use crate::sync::UPSafeCell;
use crate::task::{block_current_and_run_next, current_task, wakeup_task, TaskControlBlock};
use alloc::{collections::VecDeque, sync::Arc};

/// semaphore structure
pub struct Semaphore {
    /// semaphore inner
    pub inner: UPSafeCell<SemaphoreInner>,
}

pub struct SemaphoreInner {
    pub count: isize,
    pub wait_queue: VecDeque<Arc<TaskControlBlock>>,
    // CH8 ADDED
    origin: isize,
    pub get_queue: VecDeque<usize>,
    // CH8 ADDED
}

impl Semaphore {
    /// Create a new semaphore
    pub fn new(res_count: usize) -> Self {
        trace!("kernel: Semaphore::new");
        Self {
            inner: unsafe {
                UPSafeCell::new(SemaphoreInner {
                    count: res_count as isize,
                    wait_queue: VecDeque::new(),
                    // CH8 ADDED
                    origin: res_count as isize,
                    get_queue: VecDeque::new(),
                    // CH8 ADDED
                })
            },
        }
    }

    /// up operation of semaphore
    pub fn up(&self) {
        trace!("kernel: Semaphore::up");
        let mut inner = self.inner.exclusive_access();
        inner.count += 1;
        if inner.count <= 0 {
            if let Some(task) = inner.wait_queue.pop_front() {
                // CH8 ADDED
                inner.get_queue.pop_front();
                // CH8 ADDED
                wakeup_task(task);
            }
        }
    }

    /// down operation of semaphore
    pub fn down(&self) {
        trace!("kernel: Semaphore::down");
        let mut inner = self.inner.exclusive_access();
        inner.count -= 1;
        if inner.count < 0 {
            inner.wait_queue.push_back(current_task().unwrap());
            drop(inner);
            block_current_and_run_next();
        }
        // CH8 ADDED
        else{
            let tid = current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid;
            inner.get_queue.push_back(tid);
        }
        // CH8 ADDED
    }

    // CH8 ADDED
    /// get origin
    pub fn get_origin(&self) -> isize {
        let inner = self.inner.exclusive_access();
        inner.origin
    }

    /// get remain
    pub fn get_remain(&self) -> isize {
        let inner = self.inner.exclusive_access();
        if inner.count < 0{
            0
        }else{
            inner.count
        }
    }

    /// get remain
    pub fn get_alloc(&self) -> isize {
        let inner = self.inner.exclusive_access();
        inner.origin - if inner.count < 0{
            0
        }else{
            inner.count
        }
    }
    // CH8 ADDED
}
