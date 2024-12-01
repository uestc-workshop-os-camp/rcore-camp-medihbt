//! File and filesystem-related syscalls
use crate::fs::{link_file, open_file, unlink_file, OpenFlags, Stat};
use crate::mm::utils::copy_obj_to_user;
use crate::mm::{translated_byte_buffer, translated_str, UserBuffer};
use crate::task::{current_task, current_user_token, read_current_tcb};

pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
    trace!("kernel:pid[{}] sys_write", current_task().unwrap().pid.0);
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        if !file.writable() {
            return -1;
        }
        let file = file.clone();
        // release current task TCB manually to avoid multi-borrow
        drop(inner);
        file.write(UserBuffer::new(translated_byte_buffer(token, buf, len))) as isize
    } else {
        -1
    }
}

pub fn sys_read(fd: usize, buf: *const u8, len: usize) -> isize {
    trace!("kernel:pid[{}] sys_read", current_task().unwrap().pid.0);
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        warn!("Kernel: fd overflow ({} >= {})", fd, inner.fd_table.len());
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        let file = file.clone();
        if !file.readable() {
            warn!("Kernel: fd({}) NOT readable", fd);
            return -1;
        }
        // release current task TCB manually to avoid multi-borrow
        drop(inner);
        trace!("kernel: sys_read .. file.read");
        file.read(UserBuffer::new(translated_byte_buffer(token, buf, len))) as isize
    } else {
        warn!("Kernel: fd({}) is NONE", fd);
        -1
    }
}

pub fn sys_open(path: *const u8, flags: u32) -> isize {
    trace!("kernel:pid[{}] sys_open", current_task().unwrap().pid.0);
    let task = current_task().unwrap();
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(inode) = open_file(path.as_str(), OpenFlags::from_bits(flags).unwrap()) {
        let mut inner = task.inner_exclusive_access();
        let fd = inner.alloc_fd();
        inner.fd_table[fd] = Some(inode);
        fd as isize
    } else {
        warn!("Kernel: file `{}` unexisted", path);
        -1
    }
}

pub fn sys_close(fd: usize) -> isize {
    trace!("kernel:pid[{}] sys_close", current_task().unwrap().pid.0);
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if inner.fd_table[fd].is_none() {
        return -1;
    }
    inner.fd_table[fd].take();
    0
}

/// YOUR JOB: Implement fstat.
pub fn sys_fstat(fd: usize, _st: *mut Stat) -> isize {
    trace!(
        "kernel:pid[{}] sys_fstat NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    let stat_res = read_current_tcb(&mut |_pid, tcb| {
        if fd >= tcb.fd_table.len() {
            None
        } else if let Some(ref file) = tcb.fd_table[fd] {
            Some(file.stat())
        } else {
            None
        }
    });
    if let Some(stat) = stat_res {
        unsafe { copy_obj_to_user(_st, &stat); }
        0
    } else {
        -1
    }
}

/// YOUR JOB: Implement linkat.
pub fn sys_linkat(_old_name: *const u8, _new_name: *const u8) -> isize {
    trace!(
        "kernel:pid[{}] sys_linkat NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    warn!("link start");
    let token = current_user_token();
    let from_name = translated_str(token, _old_name);
    let to_name = translated_str(token, _new_name);

    match link_file(from_name.as_str(), to_name.as_str()) {
        Some(_) => 0,
        None    => -1
    }
}

/// YOUR JOB: Implement unlinkat.
pub fn sys_unlinkat(_name: *const u8) -> isize {
    trace!(
        "kernel:pid[{}] sys_unlinkat NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    trace!("unlink start");
    let token = current_user_token();
    let filename = translated_str(token, _name);

    match unlink_file(filename.as_str()) {
        Ok(_)  => {
            warn!("unlink success");
            0
        },
        Err(s) => {
            warn!("unlink err: {s}");
            -1
        }
    }
}
