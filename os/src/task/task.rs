//! Types related to task management & Functions for completely changing TCB

use super::id::TaskUserRes;
use super::{kstack_alloc, KernelStack, ProcessControlBlock, TaskContext};
use crate::trap::TrapContext;
use crate::{mm::PhysPageNum, sync::UPSafeCell};
use alloc::sync::{Arc, Weak};
use core::cell::{Ref, RefMut};

/// Task control block structure
pub struct TaskControlBlock {
    /// immutable
    pub process: Weak<ProcessControlBlock>,
    /// Kernel stack corresponding to PID
    pub kstack: KernelStack,
    /// mutable
    inner: UPSafeCell<TaskControlBlockInner>,
}

impl TaskControlBlock {
    /// Get the mutable reference of the inner TCB
    pub fn inner_exclusive_access(&self) -> RefMut<'_, TaskControlBlockInner> {
        self.inner.exclusive_access()
    }
    /// Get the immutable reference of the inner TCB
    pub fn inner_ro_access(&self) -> Ref<'_, TaskControlBlockInner> {
        self.inner.ro_access()
    }
    /// Get the address of app's page table
    pub fn get_user_token(&self) -> usize {
        let process = self.process.upgrade().unwrap();
        let inner = process.inner_exclusive_access();
        inner.memory_set.token()
    }
    /// Get task ID in this process task collection.
    pub fn gettid(&self)-> Option<usize> {
        match &self.inner_ro_access().res {
            Some(res) => Some(res.tid),
            None => None
        }
    }
}

pub struct TaskControlBlockInner {
    pub res: Option<TaskUserRes>,
    /// The physical page number of the frame where the trap context is placed
    pub trap_cx_ppn: PhysPageNum,
    /// Save task context
    pub task_cx: TaskContext,

    /// Maintain the execution status of the current process
    pub task_status: TaskStatus,
    /// It is set when active exit or execution error occurs
    pub exit_code: Option<i32>,

    /// Schedule infomation
    pub sched_info: SchedInfo,
}

impl TaskControlBlockInner {
    pub fn get_trap_cx(&self) -> &'static mut TrapContext {
        self.trap_cx_ppn.get_mut()
    }

    #[allow(unused)]
    fn get_status(&self) -> TaskStatus {
        self.task_status
    }
}

#[derive(Clone, Copy)]
/// Data related to stride scheduling algorithm
pub struct SchedInfo {
    /// Priority
    _priority: usize,

    /// Pass (equals to STRIDE_BASE / priority)
    _pass:     usize,

    /// Current stride
    _stride:   usize,
}

impl SchedInfo {
    /// default priority
    pub const DEFAULT_PRIORITY:  usize  = 16;

    /// default pass
    pub const DEFAULT_PASS_BASE: usize = 65537;

    /// default pass, like DEFAULT_PASS_BASE / DEFAULT_PRIORITY.
    pub const DEFAULT_PASS:      usize = 4096;

    /// New SchedInfo instance for new process.
    pub fn new()-> Self {
        Self {
            _priority: Self::DEFAULT_PRIORITY,
            _pass:     Self::DEFAULT_PASS,
            _stride: 0,
        }
    }

    /// New SchedInfo instance for new process, with priority `prio`.
    pub fn with_priority(prio: usize)-> Self {
        Self {
            _priority: prio,
            // Most of prioroties are running in DEFAULT_PRIORITY,
            // use this selection to decrease dividing
            _pass: if prio == Self::DEFAULT_PRIORITY {
                    Self::DEFAULT_PASS
                } else {
                    Self::DEFAULT_PASS_BASE / prio
                },
            _stride: 0
        }
    }

    /// Used in fork(): clone a schedinfo from parent process
    pub fn clone_from(old_sched_info: &Self)-> Self {
        Self {
            _priority: old_sched_info._priority,
            _pass:     old_sched_info._pass,
            _stride:   0
        }
    }

    /// Reset schedule infomation. This is triggered when calling exec().
    pub fn full_reset(&mut self) {
        *self = Self::new()
    }

    /// Trivial getter: stride
    pub fn get_stride(&self)-> usize { self._stride }
    /// Reset: stride
    pub fn reset_stride(&mut self)-> &mut Self {
        self._stride = 0; self
    }

    /// Trivial getter: pass
    pub fn get_pass(&self)-> usize { self._pass }

    /// Trivial getter: priority
    pub fn get_priority(&self)-> usize { self._priority }
    /// Setter: priority
    ///
    /// This updates `_pass` field
    pub fn set_priority(&mut self, priority: usize)-> &mut Self {
        self._priority = priority;
        self._pass = match priority {
            Self::DEFAULT_PRIORITY => Self::DEFAULT_PASS,
            _ => Self::DEFAULT_PASS_BASE / priority,
        };
        self
    }

    /// Update schedule infomation on process run
    pub fn update(&mut self, dtime: usize)-> &mut Self {
        self._stride += self._pass as usize * dtime;
        self
    }
}

impl TaskControlBlock {
    /// Create a new task
    pub fn new(
        process: Arc<ProcessControlBlock>,
        ustack_base: usize,
        alloc_user_res: bool,
    ) -> Self {
        let res = TaskUserRes::new(Arc::clone(&process), ustack_base, alloc_user_res);
        let trap_cx_ppn = res.trap_cx_ppn();
        let kstack = kstack_alloc();
        let kstack_top = kstack.get_top();
        Self {
            process: Arc::downgrade(&process),
            kstack,
            inner: unsafe {
                UPSafeCell::new(TaskControlBlockInner {
                    res: Some(res),
                    trap_cx_ppn,
                    task_cx: TaskContext::goto_trap_return(kstack_top),
                    task_status: TaskStatus::Ready,
                    exit_code: None,
                    sched_info: SchedInfo::new(),
                })
            },
        }
    }
}

#[derive(Copy, Clone, PartialEq)]
/// The execution status of the current process
pub enum TaskStatus {
    /// ready to run
    Ready,
    /// running
    Running,
    /// blocked
    Blocked,
}
