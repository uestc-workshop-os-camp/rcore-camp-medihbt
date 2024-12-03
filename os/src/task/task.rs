//! Types related to task management & Functions for completely changing TCB
use super::TaskContext;
use super::{kstack_alloc, pid_alloc, KernelStack, PidHandle};
use crate::config::{MAX_SYSCALL_NUM, TRAP_CONTEXT_BASE};
use crate::mm::{MemorySet, PhysPageNum, VirtAddr, KERNEL_SPACE};
use crate::sync::UPSafeCell;
use crate::timer;
use crate::trap::{trap_handler, TrapContext};
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use core::cell::{Ref, RefMut};
use core::cmp::Ordering;

/// Task control block structure
///
/// Directly save the contents that will not change during running
pub struct TaskControlBlock {
    // Immutable
    /// Process identifier
    pub pid: PidHandle,

    /// Kernel stack corresponding to PID
    pub kernel_stack: KernelStack,

    /// Mutable
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
        let inner = self.inner_exclusive_access();
        inner.memory_set.token()
    }
    /// Trivial getter for priority.
    pub fn get_priority(&self) -> usize {
        self.inner.ro_access().sched_info.get_priority()
    }
}

/// Inner TCB, which contains inner mutability in a readonly TCB reference.
pub struct TaskControlBlockInner {
    /// The physical page number of the frame where the trap context is placed
    pub trap_cx_ppn: PhysPageNum,

    /// Application data can only appear in areas
    /// where the application address space is lower than base_size
    pub base_size: usize,

    /// Save task context
    pub task_cx: TaskContext,

    /// Maintain the execution status of the current process
    pub task_status: TaskStatus,

    /// Application address space
    pub memory_set: MemorySet,

    /// Parent process of the current process.
    /// Weak will not affect the reference count of the parent
    pub parent: Option<Weak<TaskControlBlock>>,

    /// A vector containing TCBs of all child processes of the current process
    pub children: Vec<Arc<TaskControlBlock>>,

    /// It is set when active exit or execution error occurs
    pub exit_code: i32,

    /// Heap bottom
    pub heap_bottom: usize,

    /// Program break
    pub program_brk: usize,

    /// Task statistics
    pub statistics: TcbStatistics,

    /// Task scheduling infomation
    pub sched_info: SchedInfo,
}

impl TaskControlBlockInner {
    /// get the trap context
    pub fn get_trap_cx(&self) -> &'static mut TrapContext {
        self.trap_cx_ppn.get_mut()
    }
    /// get the user token
    pub fn get_user_token(&self) -> usize {
        self.memory_set.token()
    }
    fn get_status(&self) -> TaskStatus {
        self.task_status
    }
    /// Check if a task is zombie.
    pub fn is_zombie(&self) -> bool {
        self.get_status() == TaskStatus::Zombie
    }

    /// Slot method: activate
    pub fn on_activate(&mut self) {
        self.task_status = TaskStatus::Running;
        self.statistics.on_activate();
    }

    /// Slot method: deactivate. Run on this task changing state from 'RUNNING' to other state.
    pub fn on_deactivate(&mut self) {
        self.statistics.on_deactivate();
        self.sched_info.update(self.statistics.get_last_run_time());
    }

    /// Slot method: Run on this TCB is killed.
    pub fn on_dead(&mut self, task_status: TaskStatus, exit_code: i32) {
        self.on_deactivate();
        self.task_status = task_status;
        self.exit_code   = exit_code;
    }
}

#[derive(Clone, Copy)]
/// Task runtime statistics infomation.
pub struct TcbStatistics {
    /// Startup Time
    pub startup_time: usize,

    /// Syscall Infomation
    pub syscall_times: [u32; MAX_SYSCALL_NUM],

    /// Run time Infomation: Last-acativated time
    pub last_activate_time: usize,

    /// Run time infomation: Last-stopped time
    pub last_deactivate_time: usize,
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

/// Stride object, which lives in ready queue.
#[derive(Clone)]
pub struct Stride(usize, Arc<TaskControlBlock>);

impl SchedInfo {
    /// default priority
    pub const DEFAULT_PRIORITY: usize  = 16;

    /// default pass
    pub const DEFAULT_BIG_STRIDE: usize = 65537;

    /// default pass, like DEFAULT_BIG_STRIDE / DEFAULT_PRIORITY.
    pub const DEFAULT_PASS: usize = Self::DEFAULT_BIG_STRIDE / Self::DEFAULT_PRIORITY;

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
                    Self::DEFAULT_BIG_STRIDE / prio
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
            _ => Self::DEFAULT_BIG_STRIDE / priority,
        };
        self
    }

    /// Update schedule infomation on process run
    pub fn update(&mut self, dtime: usize)-> &mut Self {
        self._stride += self._pass as usize * dtime;
        self
    }
}

impl Ord for Stride {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.0.cmp(&other.0) {
            Ordering::Less    => Ordering::Greater,
            Ordering::Equal   => Ordering::Equal,
            Ordering::Greater => Ordering::Less,
        }
    }
}

impl PartialOrd for Stride {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match self.0.cmp(&other.0) {
            Ordering::Less    => Some(Ordering::Greater),
            Ordering::Equal   => Some(Ordering::Equal),
            Ordering::Greater => Some(Ordering::Less),
        }
    }
}

impl PartialEq for Stride {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl Eq for Stride {
    fn assert_receiver_is_total_eq(&self) {}
}

impl Stride {
    /// Convert this Stride into TCB
    pub fn into_tcb(&self) -> Arc<TaskControlBlock> {
        self.1.clone()
    }
}

impl TcbStatistics {
    /// Create an enpty task statistics item
    pub fn empty()-> Self {
        Self {
            startup_time: 0,
            syscall_times: [0; MAX_SYSCALL_NUM],
            last_activate_time: 0,
            last_deactivate_time: 0
        }
    }

    /// React on process startup
    pub fn on_activate(&mut self) {
        if self.startup_time == 0 {
            self.startup_time = timer::get_time();
        }
        self.last_activate_time = timer::get_time();
    }

    /// React on deactivate
    pub fn on_deactivate(&mut self) {
        self.last_deactivate_time = timer::get_time();
    }

    /// Time between last activate and deactivate
    pub fn get_last_run_time(&self) -> usize {
        assert!(self.last_deactivate_time >= self.last_activate_time);
        self.last_deactivate_time - self.last_activate_time
    }

    /// React on syscall
    pub fn on_syscall(&mut self, syscall_id: usize) {
        self.syscall_times[syscall_id] += 1;
    }

    /// Reset this
    pub fn reset(&mut self) {
        self.startup_time  = 0;
        self.syscall_times = [0; MAX_SYSCALL_NUM];
    }

    /// React on executing this
    pub fn on_exec(&mut self) {
        self.reset();
    }
}

impl TaskControlBlock {
    /// Create a new process
    ///
    /// At present, it is only used for the creation of initproc
    pub fn new(elf_data: &[u8]) -> Self {
        // memory_set with elf program headers/trampoline/trap context/user stack
        let (memory_set, user_sp, entry_point) = MemorySet::from_elf(elf_data);
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT_BASE).into())
            .unwrap()
            .ppn();
        // alloc a pid and a kernel stack in kernel space
        let pid_handle = pid_alloc();
        let kernel_stack = kstack_alloc();
        let kernel_stack_top = kernel_stack.get_top();
        // push a task context which goes to trap_return to the top of kernel stack
        let task_control_block = Self {
            pid: pid_handle,
            kernel_stack,
            inner: unsafe {
                UPSafeCell::new(TaskControlBlockInner {
                    trap_cx_ppn,
                    base_size: user_sp,
                    task_cx: TaskContext::goto_trap_return(kernel_stack_top),
                    task_status: TaskStatus::Ready,
                    memory_set,
                    parent: None,
                    children: Vec::new(),
                    exit_code: 0,
                    heap_bottom: user_sp,
                    program_brk: user_sp,
                    statistics:  TcbStatistics::empty(),
                    sched_info:  SchedInfo::new(),
                })
            },
        };
        // prepare TrapContext in user space
        let trap_cx = task_control_block.inner_exclusive_access().get_trap_cx();
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            kernel_stack_top,
            trap_handler as usize,
        );
        task_control_block
    }

    /// Load a new elf to replace the original application address space and start execution
    pub fn exec(&self, elf_data: &[u8]) {
        // memory_set with elf program headers/trampoline/trap context/user stack
        let (memory_set, user_sp, entry_point) = MemorySet::from_elf(elf_data);
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT_BASE).into())
            .unwrap()
            .ppn();

        // **** access current TCB exclusively
        let mut inner = self.inner_exclusive_access();
        // substitute memory_set
        inner.memory_set = memory_set;
        // update trap_cx ppn
        inner.trap_cx_ppn = trap_cx_ppn;
        // initialize base_size
        inner.base_size = user_sp;
        // reset statistics data
        inner.statistics.on_exec();
        // reset schedule data
        inner.sched_info.full_reset();
        // initialize trap_cx
        let trap_cx = inner.get_trap_cx();
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            self.kernel_stack.get_top(),
            trap_handler as usize,
        );
        // **** release inner automatically
    }

    /// parent process fork the child process
    pub fn fork(self: &Arc<Self>) -> Arc<Self> {
        // ---- access parent PCB exclusively
        let mut parent_inner = self.inner_exclusive_access();
        // copy user space(include trap context)
        let memory_set = MemorySet::from_existed_user(&parent_inner.memory_set);
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT_BASE).into())
            .unwrap()
            .ppn();
        // alloc a pid and a kernel stack in kernel space
        let pid_handle = pid_alloc();
        let kernel_stack = kstack_alloc();
        let kernel_stack_top = kernel_stack.get_top();
        let task_control_block = Arc::new(TaskControlBlock {
            pid: pid_handle,
            kernel_stack,
            inner: unsafe {
                UPSafeCell::new(TaskControlBlockInner {
                    trap_cx_ppn,
                    base_size: parent_inner.base_size,
                    task_cx: TaskContext::goto_trap_return(kernel_stack_top),
                    task_status: TaskStatus::Ready,
                    memory_set,
                    parent: Some(Arc::downgrade(self)),
                    children: Vec::new(),
                    exit_code: 0,
                    heap_bottom: parent_inner.heap_bottom,
                    program_brk: parent_inner.program_brk,
                    statistics:  TcbStatistics::empty(),
                    sched_info:  SchedInfo::clone_from(&parent_inner.sched_info),
                })
            },
        });
        // add child
        parent_inner.children.push(task_control_block.clone());
        // modify kernel_sp in trap_cx
        // **** access child PCB exclusively
        let trap_cx = task_control_block.inner_exclusive_access().get_trap_cx();
        trap_cx.kernel_sp = kernel_stack_top;
        // return
        task_control_block
        // **** release child PCB
        // ---- release parent PCB
    }

    /// spawn a new process with elf data `app_elf`
    pub fn spawn(self: &Arc<Self>, app_elf: &[u8])-> Arc<Self> {
        let ret = Arc::new(Self::new(app_elf));
        ret.inner_exclusive_access().parent = Some(Arc::downgrade(&self));
        self.inner_exclusive_access().children.push(ret.clone());
        ret
    }

    /// get pid of process
    pub fn getpid(&self) -> usize {
        self.pid.0
    }

    /// change the location of the program break. return None if failed.
    pub fn change_program_brk(&self, size: i32) -> Option<usize> {
        let mut inner = self.inner_exclusive_access();
        let heap_bottom = inner.heap_bottom;
        let old_break = inner.program_brk;
        let new_brk = inner.program_brk as isize + size as isize;
        if new_brk < heap_bottom as isize {
            return None;
        }
        let result = if size < 0 {
            inner
                .memory_set
                .shrink_to(VirtAddr(heap_bottom), VirtAddr(new_brk as usize))
        } else {
            inner
                .memory_set
                .append_to(VirtAddr(heap_bottom), VirtAddr(new_brk as usize))
        };
        if result {
            inner.program_brk = new_brk as usize;
            Some(old_break)
        } else {
            None
        }
    }

    /// React on activation of this process. This method sets task status to
    /// `Running` and updates scheduling info.
    pub fn on_activate(&self)-> &Self {
        let mut inner = self.inner_exclusive_access();
        inner.on_activate();
        self
    }

    /// Run on deactivation of this task.
    pub fn on_deactivate(&self, new_status: TaskStatus)-> &Self {
        let mut inner = self.inner_exclusive_access();
        inner.task_status = new_status;
        inner.on_deactivate();
        self
    }

    /// Convert this into stride
    pub fn into_stride(self: &Arc<Self>) -> Stride {
        Stride(self.inner_ro_access().sched_info._stride, self.clone())
    }
}

#[derive(Copy, Clone, PartialEq)]
/// task status: UnInit, Ready, Running, Exited
pub enum TaskStatus {
    /// uninitialized
    UnInit,
    /// ready to run
    Ready,
    /// running
    Running,
    /// exited
    Zombie,
}
