//! Process management syscalls
use crate::{
    config::{CLOCK_FREQ, MAX_SYSCALL_NUM}, mm::{self, utils::copy_obj_to_user}, task::{
        change_program_brk, exit_current_and_run_next, read_current_tcb, suspend_current_and_run_next, TaskStatus
    }, timer
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
    let time_us = timer::get_time_us();
    let sec  = time_us / 1_000_000;
    let usec = time_us % 1_000_000;
    unsafe {
        copy_obj_to_user(_ts, &TimeVal {
            sec, usec,
        });
    }
    0
}

/// YOUR JOB: Finish sys_task_info to pass testcases
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TaskInfo`] is splitted by two pages ?
pub fn sys_task_info(_ti: *mut TaskInfo) -> isize {
    let mut curr_task  = TaskInfo {
        status:        TaskStatus::Exited,
        syscall_times: [0; MAX_SYSCALL_NUM],
        time:          0
    };
    read_current_tcb(|_tid, tcb| {
        curr_task.status = tcb.task_status;
        curr_task.syscall_times.copy_from_slice(tcb.syscall_times.as_slice());
        let dtime_ticks = timer::get_time() - tcb.startup_time;
        curr_task.time = dtime_ticks * 1000 / CLOCK_FREQ;
    });
    unsafe { copy_obj_to_user(_ti, &curr_task); }
    trace!("kernel: sys_task_info");
    0
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(start: usize, len: usize, prot: usize) -> isize {
    trace!("kernel: sys_mmap");
    mm::utils::mmap_handle::do_mmap(start, len, prot)
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(start: usize, len: usize) -> isize {
    trace!("kernel: sys_munmap NOT IMPLEMENTED YET!");
    mm::utils::mmap_handle::do_munmap(start, len)
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
