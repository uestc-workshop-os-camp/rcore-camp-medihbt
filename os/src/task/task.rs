//! Types related to task management

use crate::config::MAX_SYSCALL_NUM;

use super::{TaskContext, TASK_MANAGER};

/// The task control block (TCB) of a task.
#[derive(Copy, Clone)]
pub struct TaskControlBlock {
    /// The task status in it's lifecycle
    pub task_status: TaskStatus,
    /// The task context
    pub task_cx: TaskContext,
    /// Statistics
    /// How many times each syscall is called
    pub syscall_times: [u32; MAX_SYSCALL_NUM],
    /// Run time in user mode
    pub user_time: usize,
    /// Run time in kernel mode
    pub kernel_time: usize,
    /// Startup time in ticks
    pub startup_time: usize,
}

impl TaskControlBlock {
    /// Create an empty TCB struct
    pub fn empty() -> Self {
        Self {
            task_status:    TaskStatus::UnInit,
            task_cx:        TaskContext::zero_init(),
            syscall_times:  [0; MAX_SYSCALL_NUM],
            user_time:      0,
            kernel_time:    0,
            startup_time:   0
        }
    }

    /// Update run time of a TCB by adding kernel time
    ///  & user time with dk / du
    pub fn add_run_time(&mut self, dk: usize, du: usize) {
        self.kernel_time += dk;
        self.user_time   += du;
    }

    /// Update user-mode time. This method can be called only
    /// if the OS has just entered trap.
    pub fn update_user_time(&mut self) {
        self.user_time += TASK_MANAGER.stopwatch_reboot_get_dt();
    }

    /// Update kernel-mode time. This method can be called
    /// only if the process is going to be switched to a new
    /// task or the process is going to return to user mode
    /// from a syscall.
    pub fn update_kernel_time(&mut self) {
        self.kernel_time += TASK_MANAGER.stopwatch_reboot_get_dt();
    }

    /// Update syscall time of a TCB. This method can only
    /// happen in a syscall.
    pub fn add_syscall(&mut self, syscall_id: usize) {
        self.syscall_times[syscall_id] += 1;
    }
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
