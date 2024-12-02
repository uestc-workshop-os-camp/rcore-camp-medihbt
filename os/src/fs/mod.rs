//! File trait & inode(dir, file, pipe, stdin, stdout)

mod inode;
mod pipe;
mod stdio;

use crate::mm::UserBuffer;

/// trait File for all file types
pub trait File: Send + Sync {
    /// the file readable?
    fn readable(&self) -> bool;
    /// the file writable?
    fn writable(&self) -> bool;
    /// read from the file to buf, return the number of bytes read
    fn read(&self, buf: UserBuffer) -> usize;
    /// write to the file from buf, return the number of bytes written
    fn write(&self, buf: UserBuffer) -> usize;
    /// Get stat of this file
    fn stat(&self)-> Stat;
}

/// The stat of a inode
#[repr(C)]
#[derive(Debug)]
pub struct Stat {
    /// ID of device containing file
    pub dev: u64,
    /// inode number
    pub ino: u64,
    /// file type and mode
    pub mode: StatMode,
    /// number of hard links
    pub nlink: u32,
    /// unused pad
    pad: [u64; 7],
}

impl Stat {
    /// Create a new Stat with all infomation needed.
    pub fn new(dev: u64, ino: u64, mode: StatMode, nlink: u32)-> Self {
        Self {
            dev, ino, mode, nlink,
            pad: [0u64; 7]
        }
    }
}

bitflags! {
    /// The mode of a inode
    /// whether a directory or a file
    pub struct StatMode: u32 {
        /// null
        const NULL  = 0;
        /// pipe file
        const FIFO  = 0o010000;
        /// character stream device
        const CHR   = 0o020000;
        /// directory
        const DIR   = 0o040000;
        /// block device
        const BLK   = 0o060000;
        /// ordinary regular file
        const FILE  = 0o100000;
        /// symbolic link
        const LNK   = 0o120000;
        /// socket
        const SOCK  = 0o140000;
    }
}

use inode::ROOT_INODE;
pub use inode::{list_apps, open_file, OSInode, OpenFlags};
pub use pipe::{make_pipe, Pipe};
pub use stdio::{Stdin, Stdout};


/// Link a file
pub fn link_file(from_name: &str, to_name: &str)-> Option<alloc::sync::Arc<easy_fs::Inode>> {
    warn!("link start");
    if from_name == to_name {
        warn!("Link name {} is original", to_name);
        return None;
    }
    let mut from_node = if let Some(x) = ROOT_INODE.find(from_name) {
        if x.is_dir_file().0 {
            warn!("Cannot link a directory");
            return None;
        }
        x
    } else {
        warn!("File name {} NOT FOUND", from_name);
        return None;
    };
    if ROOT_INODE.find(to_name).is_some() {
        warn!("Link name {} is existed file", to_name);
        return None;
    }

    match ROOT_INODE.hard_link(&mut from_node, to_name) {
        Ok(v) => Some(v),
        Err(s) => {
            warn!("efs: {}", s);
            None
        }
    }
}

/// unlink a file `filename`.
pub fn unlink_file(filename: &str)-> Result<(), &'static str> {
    let mut from_node = if let Some(x) = ROOT_INODE.find(filename) {
        if x.is_dir_file().0 {
            return Err("Cannot link a directory");
        }
        x
    } else {
        warn!("File name {} NOT FOUND", filename);
        return Err("File name NOT FOUND");
    };
    ROOT_INODE.hard_unlink(&mut from_node, |str| {
        warn!("efs message: {}", str);
    })
}
