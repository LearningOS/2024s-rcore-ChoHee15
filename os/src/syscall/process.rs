//! Process management syscalls
use crate::{
    config::MAX_SYSCALL_NUM,
    task::{
        change_program_brk, exit_current_and_run_next, suspend_current_and_run_next, TaskStatus,
    },
};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// Task information
#[allow(dead_code)]
pub struct TaskInfo {
    /// Task status in it's life cycle
    status: TaskStatus,
    /// The numbers of syscall called by task
    syscall_times: [u32; MAX_SYSCALL_NUM],
    /// Total running time of task
    time: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(_exit_code: i32) -> ! {
    trace!("kernel: sys_exit");
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = crate::timer::get_time_us();
    unsafe {
        let mut tmp = crate::mm::translated_byte_buffer(
            crate::task::current_user_token(), _ts as *const u8, core::mem::size_of::<TimeVal>());
        let p = tmp[0].as_mut_ptr() as *mut TimeVal;
        // let p: &mut TimeVal = core::mem::transmute(&mut tmp[0]); 
        (*p).sec = us / 1_000_000;
        (*p).usec = us % 1_000_000;
    }
    0
}

/// YOUR JOB: Finish sys_task_info to pass testcases
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TaskInfo`] is splitted by two pages ?
pub fn sys_task_info(_ti: *mut TaskInfo) -> isize {
    trace!("kernel: sys_task_info");
    unsafe {
        let mut tmp = crate::mm::translated_byte_buffer(
            crate::task::current_user_token(), _ti as *const u8, core::mem::size_of::<TimeVal>());
        let p = tmp[0].as_mut_ptr() as *mut TaskInfo;
        // let p: &mut TimeVal = core::mem::transmute(&mut tmp[0]); 
        (*p).status = TaskStatus::Running;
        (*p).syscall_times = crate::task::get_info_syscall();
        (*p).time = crate::timer::get_time_us()/1000 - crate::task::get_info_starttime();
        return 0;
    }
    // error!("sys_task_info FAILED!!!");
    // -1
}

// YOUR JOB: Implement mmap.
//TODO: dont delete whole AreaSegment when the range is just a part of that
///remove WHOLE AreaSegment which contain the range
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    // trace!("kernel: sys_mmap NOT IMPLEMENTED YET!");
    // println!("mmap : [{:#x}, {:#x})", _start, _start + _len);
    if (_start & ((1 << crate::config::PAGE_SIZE_BITS) -1)) != 0{
        return -1;
    }
    if (_port & !0x7) != 0 || (_port & 0x7) == 0 {
        return -1;
    }

    let mut perm = crate::mm::MapPermission::U;
    if _port & 0x1 != 0{
        perm |= crate::mm::MapPermission::R;
    }
    if _port & 0x2 != 0{
        perm |= crate::mm::MapPermission::W;
    }
    if _port & 0x4 != 0{
        perm |= crate::mm::MapPermission::X;
    }

    let _end = _start + _len;

    // println!("mmap aligned: [{:#x}, {:#x})", _start, _end);
    crate::task::current_add_map(_start, _end, perm)

}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    // trace!("kernel: sys_munmap NOT IMPLEMENTED YET!");
    // println!("munmap : [{:#x}, {:#x})", _start, _start + _len);
    if (_start & ((1 << crate::config::PAGE_SIZE_BITS) -1)) != 0{
        return -1;
    }

    // let _end = _start + _len - 1;
    // let _end_ceil: crate::mm::VirtAddr= crate::mm::VirtAddr::from(_start + _len - 1).ceil().into();
    // let _end_align: usize = if _end == _end_ceil.into() {
    //     _end
    // }else{
    //     Into::<usize>::into(_end_ceil) - 1
    // };
    let _end = _start + _len;

    crate::task::current_remove_map(_start, _end)
}
/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel: sys_sbrk");
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}
