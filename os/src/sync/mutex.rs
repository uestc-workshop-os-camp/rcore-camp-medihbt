//! Mutex (spin-like and blocking(sleep))

use core::usize;

use super::UPSafeCell;
use crate::task::{get_current_pid, read_current_tcb, TaskControlBlock};
use crate::task::{block_current_and_run_next, suspend_current_and_run_next};
use crate::task::{current_task, wakeup_task};
use alloc::{collections::VecDeque, sync::Arc};

/// Mutex trait
pub trait Mutex: Sync + Send {
    /// Lock the mutex
    fn lock(&self);
    /// Unlock the mutex
    fn unlock(&self);
    /// Trace deadlock
    fn try_trace_lock_is_dead(&self) -> bool {
        false
    }
}

/// Spinlock Mutex struct
pub struct MutexSpin {
    locked: UPSafeCell<bool>,
}

impl MutexSpin {
    /// Create a new spinlock mutex
    pub fn new() -> Self {
        Self {
            locked: unsafe { UPSafeCell::new(false) },
        }
    }
}

impl Mutex for MutexSpin {
    /// Lock the spinlock mutex
    fn lock(&self) {
        trace!("kernel: MutexSpin::lock");
        loop {
            let mut locked = self.locked.exclusive_access();
            if *locked {
                drop(locked);
                suspend_current_and_run_next();
                continue;
            } else {
                *locked = true;
                return;
            }
        }
    }

    fn unlock(&self) {
        trace!("kernel: MutexSpin::unlock");
        let mut locked = self.locked.exclusive_access();
        *locked = false;
    }
}

/// Blocking Mutex struct
pub struct MutexBlocking {
    inner: UPSafeCell<MutexBlockingInner>,
}

pub struct MutexBlockingInner {
    locked: bool,
    wait_queue: VecDeque<Arc<TaskControlBlock>>,
    lock_holder: usize,
}

impl MutexBlocking {
    /// Create a new blocking mutex
    pub fn new() -> Self {
        trace!("kernel: MutexBlocking::new");
        Self {
            inner: unsafe {
                UPSafeCell::new(MutexBlockingInner {
                    locked: false,
                    wait_queue: VecDeque::new(),
                    lock_holder: usize::MAX,
                })
            },
        }
    }

    /// Trace this mutex to find out whether this lock is dead.
    pub fn trace_lock_is_dead(&self, inner: &MutexBlockingInner)-> bool {
        let current_tid = current_task().unwrap().gettid().unwrap();
        if inner.lock_holder == current_tid {
            warn!("Found dead lock in pid[{}] task[{}] (inner lock holder {})",
                  get_current_pid(), current_tid, inner.lock_holder);
            return true;
        }
        match inner.wait_queue.iter().find(|t| {t.gettid() == Some(current_tid)}) {
            Some(_) => {
                warn!("Found dead lock in pid[{}] task[{}] (inner lock holder {})",
                    get_current_pid(), current_tid, inner.lock_holder);
                true
            }
            None => {
                warn!("No dead lock in pid[{}] task[{}] (inner lock holder {})",
                    get_current_pid(), current_tid, inner.lock_holder);
                false
            }
        }
    }
}

impl Mutex for MutexBlocking {
    /// lock the blocking mutex
    fn lock(&self) {
        trace!("kernel: MutexBlocking::lock");
        let mut mutex_inner = self.inner.exclusive_access();
        if mutex_inner.locked {
            mutex_inner.wait_queue.push_back(current_task().unwrap());
            drop(mutex_inner);
            block_current_and_run_next();
        } else {
            mutex_inner.locked = true;
            mutex_inner.lock_holder = current_task().unwrap().gettid().unwrap();
        }
    }

    /// unlock the blocking mutex
    fn unlock(&self) {
        trace!("kernel: MutexBlocking::unlock");
        let mut mutex_inner = self.inner.exclusive_access();
        assert!(mutex_inner.locked);
        if let Some(waking_task) = mutex_inner.wait_queue.pop_front() {
            wakeup_task(waking_task);
        } else {
            mutex_inner.locked = false;
            mutex_inner.lock_holder = usize::MAX;
        }
    }

    /// trace this lock if this is dead.
    fn try_trace_lock_is_dead(&self) -> bool {
        if read_current_tcb(|p, _| { !p.deadlock_tracing_enabled() }) {
            warn!("no need to trace mutex! pid {}", get_current_pid());
            return false;
        }
        let inner = self.inner.ro_access();
        if inner.locked == false {
            false
        } else {
            warn!("tracing mutex...");
            self.trace_lock_is_dead(&self.inner.ro_access())
        }
    }
}
