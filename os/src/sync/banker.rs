//! banker algorithm
//! by medihbt

use alloc::{sync::Arc, vec::Vec};

use super::UPSafeCell;

/// Maximum threads number.
pub const MAX_THREADS:  usize = 16;
/// Maximum resource number.
pub const MAX_RESOURCE: usize = 8;

/// A greedy banker.
#[derive(Clone, Copy)]
pub struct Banker {
    /// Allocated resources per resource per thread.
    pub allocated: [[usize; MAX_RESOURCE]; MAX_THREADS],
    /// Needed resources per resource per thread.
    pub need:      [[usize; MAX_RESOURCE]; MAX_THREADS],
    /// Available resources per resource.
    pub available: [usize; MAX_RESOURCE],
}

impl Banker {
    /// Create Banker
    pub fn new()-> Self {
        Self {
            allocated: [[0; MAX_RESOURCE]; MAX_THREADS],
            need:      [[0; MAX_RESOURCE]; MAX_THREADS],
            available: [0;  MAX_RESOURCE],
        }
    }
    /// Create a new reference-counted banker.
    pub fn new_arc_up() -> Arc<UPSafeCell<Self>> {
        Arc::new(unsafe { UPSafeCell::new(Self::new()) })
    }
    /// deep-copy this banker and put it onto heap.
    pub fn clone_arc(&self) -> Arc<UPSafeCell<Self>> {
        Arc::new(unsafe {UPSafeCell::new(self.clone())})
    }

    /// 银行家检查算法, 检查在当前状况下是否存在安全的资源申请路径
    /// 如果 &self 实现不了, 就换成 &mut self
    pub fn is_safe(&self) -> bool {
        let mut work = self.available.clone();
        let mut finish = [false; MAX_THREADS];
        let mut safe_seq = Vec::with_capacity(MAX_THREADS);

        loop {
            let mut found = false;
            for i in 0..MAX_THREADS {
                if finish[i] {
                    continue;
                }
                // self.need < self.available
                if self.need[i].iter().zip(work.iter())
                   .all(|(&need, &avail)| need <= avail) {
                    for j in 0..MAX_RESOURCE {
                        work[j] += self.allocated[i][j];
                    }
                    finish[i] = true;
                    found     = true;
                    safe_seq.push(i);
                }
            }
            if !found { break; }
        }

        if finish.iter().all(|x| *x) {
            print!("[Info] kernel: No deadlock, safe seq: {{ ");
            for thrd in safe_seq {
                print!("{thrd}, ");
            }
            println!("}} ");
            true
        } else {
            false
        }
    }

    /// Let thread T allocate a resource x
    pub fn try_allocate_one(&mut self, thread_id: usize, resource_id: usize) -> bool
    {
        if thread_id > MAX_THREADS {
            return false;
        }
        if resource_id >= MAX_RESOURCE || self.need[thread_id][resource_id] == 0 {
            return false;
        }
        if self.available[resource_id] == 0 {
            return false;
        }
        self.available[resource_id]            -= 1;
        self.allocated[thread_id][resource_id] += 1;
        self.need     [thread_id][resource_id] -= 1;

        if !self.is_safe() {
            self.available[resource_id]            += 1;
            self.allocated[thread_id][resource_id] -= 1;
            self.need     [thread_id][resource_id] += 1;
            false
        } else {
            true
        }
    }

    /// Let thread `thread_id` allocate a resource `resource_id` without check
    pub fn allocate_one_nocheck(&mut self, thread_id: usize, resource_id: usize) -> bool
    {
        if thread_id > MAX_THREADS {
            return false;
        }
        if resource_id >= MAX_RESOURCE || self.need[thread_id][resource_id] == 0 {
            return false;
        }
        if self.available[resource_id] == 0 {
            return false;
        }
        self.need     [thread_id][resource_id] -= 1;
        self.available[resource_id]            -= 1;
        self.allocated[thread_id][resource_id] += 1;
        true
    }

    /// deallocate resource
    pub fn try_deallocate_one(&mut self, thread_id: usize, resource_id: usize)-> bool {
        if thread_id > MAX_THREADS || resource_id >= MAX_RESOURCE || self.allocated[thread_id][resource_id] == 0 {
            false
        } else {
            self.available[resource_id]            += 1;
            self.allocated[thread_id][resource_id] -= 1;
            self.need     [thread_id][resource_id] += 1;
            true
        }
    }
    /// Dynamicly expend size of 'need' and .
    pub fn dyn_expand_dealloc(&mut self, thread_id: usize, resource_id: usize)-> bool {
        if thread_id > MAX_THREADS || resource_id >= MAX_RESOURCE || self.allocated[thread_id][resource_id] == 0 {
            false
        } else {
            self.available[resource_id]            += 1;
            self.allocated[thread_id][resource_id] -= 1;
            true
        }
    }
    /// set up thread and set needs.
    pub fn setup_thread(&mut self, thread_id: usize, need: &[usize; MAX_RESOURCE]) -> bool {
        if thread_id > MAX_THREADS {
            return false;
        }
        self.need[thread_id] = need.clone();
        true
    }
    /// set up resources.
    pub fn setup_resources(&mut self, resource_id: usize, max_available: usize) -> bool {
        if resource_id > MAX_RESOURCE {
            false
        } else {
            self.available[resource_id] = max_available;
            true
        }
    }
    /// destroy thread and release all resources.
    pub fn destroy_thread(&mut self, thread_id: usize) -> bool {
        if thread_id > MAX_THREADS {
            return false;
        }
        for resource_id in 0..MAX_RESOURCE {
            self.available[resource_id] += self.allocated[thread_id][resource_id];
            self.allocated[thread_id][resource_id] = 0;
            self.need     [thread_id][resource_id] = 0;
        }
        true
    }
}