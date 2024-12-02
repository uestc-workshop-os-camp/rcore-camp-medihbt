use crate::BLOCK_SZ;

use super::{
    block_cache_sync_all, get_block_cache, BlockDevice, DirEntry, DiskInode, DiskInodeType,
    EasyFileSystem, DIRENT_SZ,
};
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::{Mutex, MutexGuard};
/// Virtual filesystem layer over easy-fs
pub struct Inode {
    block_id: usize,
    block_offset: usize,
    fs: Arc<Mutex<EasyFileSystem>>,
    block_device: Arc<dyn BlockDevice>,
}

impl Inode {
    /// Create a vfs inode
    pub fn new(
        block_id: u32,
        block_offset: usize,
        fs: Arc<Mutex<EasyFileSystem>>,
        block_device: Arc<dyn BlockDevice>,
    ) -> Self {
        Self {
            block_id: block_id as usize,
            block_offset,
            fs,
            block_device,
        }
    }
    /// Call a function over a disk inode to read it
    fn read_disk_inode<V>(&self, f: impl FnOnce(&DiskInode) -> V) -> V {
        get_block_cache(self.block_id, Arc::clone(&self.block_device))
            .lock()
            .read(self.block_offset, f)
    }
    /// Call a function over a disk inode to modify it
    fn modify_disk_inode<V>(&self, f: impl FnOnce(&mut DiskInode) -> V) -> V {
        get_block_cache(self.block_id, Arc::clone(&self.block_device))
            .lock()
            .modify(self.block_offset, f)
    }
    /// Find inode under a disk inode by name
    fn find_inode_id(&self, name: &str, disk_inode: &DiskInode) -> Option<u32> {
        // assert it is a directory
        assert!(disk_inode.is_dir());
        let file_count = (disk_inode.size as usize) / DIRENT_SZ;
        let mut dirent = DirEntry::empty();
        for i in 0..file_count {
            assert_eq!(
                disk_inode.read_at(DIRENT_SZ * i, dirent.as_bytes_mut(), &self.block_device,),
                DIRENT_SZ,
            );
            if dirent.name() == name {
                return Some(dirent.inode_id() as u32);
            }
        }
        None
    }
    /// Find entry ID under current inode by inode ID
    pub fn find_entry_by_inode(&self, inode_id: u32, disk_inode: &DiskInode)-> Option<usize>
    {
        assert!(disk_inode.is_dir());
        let file_cnt = (disk_inode.size as usize) / DIRENT_SZ;
        let mut dirent = DirEntry::empty();
        for i in 0..file_cnt {
            assert_eq!(
                disk_inode.read_at(DIRENT_SZ * i, dirent.as_bytes_mut(), &self.block_device,),
                DIRENT_SZ);
            if dirent.inode_id() == inode_id {
                return Some(i);
            }
        }
        None
    }
    /// Find inode under current inode by name
    pub fn find(&self, name: &str) -> Option<Arc<Inode>> {
        let fs = self.fs.lock();
        self.read_disk_inode(|disk_inode| {
            self.find_inode_id(name, disk_inode).map(|inode_id| {
                let (block_id, block_offset) = fs.get_disk_inode_pos(inode_id);
                Arc::new(Self::new(
                    block_id,
                    block_offset,
                    self.fs.clone(),
                    self.block_device.clone(),
                ))
            })
        })
    }
    /// Increase the size of a disk inode
    fn increase_size(
        &self,
        new_size: u32,
        disk_inode: &mut DiskInode,
        fs: &mut MutexGuard<EasyFileSystem>,
    ) {
        if new_size < disk_inode.size {
            return;
        }
        let blocks_needed = disk_inode.blocks_num_needed(new_size);
        let mut v: Vec<u32> = Vec::new();
        for _ in 0..blocks_needed {
            v.push(fs.alloc_data());
        }
        disk_inode.increase_size(new_size, v, &self.block_device);
    }
    fn decrease_size(&self, new_size: u32, disk_inode: &mut DiskInode, fs: &mut MutexGuard<EasyFileSystem>)
    {
        if new_size > disk_inode.size {
            return;
        }
        let v = disk_inode.dealloc_to(new_size, &self.block_device);
        for block_id in v {
            fs.dealloc_data(block_id);
        }
    }
    /// Create inode under current inode by name
    pub fn create(&self, name: &str) -> Option<Arc<Inode>> {
        let mut fs = self.fs.lock();
        let op = |root_inode: &DiskInode| {
            // assert it is a directory
            assert!(root_inode.is_dir());
            // has the file been created?
            self.find_inode_id(name, root_inode)
        };
        if self.read_disk_inode(op).is_some() {
            return None;
        }
        // create a new file
        // alloc a inode with an indirect block
        let new_inode_id = fs.alloc_inode();
        // initialize inode
        let (new_inode_block_id, new_inode_block_offset) = fs.get_disk_inode_pos(new_inode_id);
        get_block_cache(new_inode_block_id as usize, Arc::clone(&self.block_device))
            .lock()
            .modify(new_inode_block_offset, |new_inode: &mut DiskInode| {
                new_inode.initialize(DiskInodeType::File);
            });
        self.modify_disk_inode(|root_inode| {
            // append file in the dirent
            let file_count = (root_inode.size as usize) / DIRENT_SZ;
            let new_size = (file_count + 1) * DIRENT_SZ;
            // increase size
            self.increase_size(new_size as u32, root_inode, &mut fs);
            // write dirent
            let dirent = DirEntry::new(name, new_inode_id);
            root_inode.write_at(
                file_count * DIRENT_SZ,
                dirent.as_bytes(),
                &self.block_device,
            );
        });

        let (block_id, block_offset) = fs.get_disk_inode_pos(new_inode_id);
        block_cache_sync_all();
        // return inode
        Some(Arc::new(Self::new(
            block_id,
            block_offset,
            self.fs.clone(),
            self.block_device.clone(),
        )))
        // release efs lock automatically by compiler
    }

    /// Make a hard link of file `From` to `To`.
    /// This operation increases reference count of file `From`.
    pub fn hard_link(&self, from: &mut Arc<Inode>, to_name: &str)-> Result<Arc<Inode>, &'static str> {
        assert!(self.is_dir_file().0);
        if self.read_disk_inode(|dir| { self.find_inode_id(to_name, dir) }).is_some() {
            return Err("Link name is existed file");
        }
        let from_id = from.get_id();
        from.modify_disk_inode(|fdi| { fdi.refthis(); });
        self.modify_disk_inode(|dir| {
            let old_size = dir.size;
            let _file_cnt = old_size / DIRENT_SZ as u32;
            let new_size = old_size + DIRENT_SZ as u32;
            self.increase_size(new_size, dir, &mut self.fs.lock());
            assert_eq!(new_size, dir.size);
            let to_entry = DirEntry::new(to_name, from_id as u32);
            dir.write_at(old_size as usize, to_entry.as_bytes(), &self.block_device);
        });
        block_cache_sync_all();
        Ok(from.clone())
    }
    /// Unlink a file `file`.
    pub fn hard_unlink(&self, file: &mut Arc<Inode>, _printf: impl Fn(&str))-> Result<(), &'static str> {
        _printf("entry");
        if !self.is_dir_file().0 {
            return Err("Inode NOT directory");
        }
        if file.get_ref_count() == 0 {
            return Err("Inode double free");
        }
        let alive = file.modify_disk_inode(|di| { di.unref() });
        _printf("disk inode unref");
        if !alive {
            // file.clear();
            _printf("clear content");
            let file_inode_id = file.get_id() as u32;
            _printf("file inode id");
            let res = self.modify_disk_inode(|dino| {
                let self_size = dino.size;
                let nentries  = self_size / DIRENT_SZ as u32;
                let ent_back  = nentries - 1;
                let file_ent  = self.find_entry_by_inode(file_inode_id, dino);
                _printf("find file_ent entry node");
                let file_ent = if let Some(x) = file_ent { x } else {
                    return Err("CANNOT fild file entry");
                } as u32;
                if ent_back != file_ent {
                    let mut dirent: DirEntry = DirEntry::empty();
                    dino.read_at((ent_back as usize) * DIRENT_SZ, dirent.as_bytes_mut(), &self.block_device);
                    dino.write_at((file_ent as usize) * DIRENT_SZ, dirent.as_bytes(), &self.block_device);
                    // this[file_end] = this[ent_back]
                    _printf("dirent swap");
                }
                self.decrease_size(self_size - DIRENT_SZ as u32, dino, &mut self.fs.lock());
                _printf("decrease size");
                return Ok(());
            });
            if res.is_err() {
                return res;
            }
        }
        block_cache_sync_all();
        Ok(())
    }
    /// List inodes under current inode
    pub fn ls(&self) -> Vec<String> {
        let _fs = self.fs.lock();
        self.read_disk_inode(|disk_inode| {
            let file_count = (disk_inode.size as usize) / DIRENT_SZ;
            let mut v: Vec<String> = Vec::new();
            for i in 0..file_count {
                let mut dirent = DirEntry::empty();
                assert_eq!(
                    disk_inode.read_at(i * DIRENT_SZ, dirent.as_bytes_mut(), &self.block_device,),
                    DIRENT_SZ,
                );
                v.push(String::from(dirent.name()));
            }
            v
        })
    }
    /// Read data from current inode
    pub fn read_at(&self, offset: usize, buf: &mut [u8]) -> usize {
        let _fs = self.fs.lock();
        self.read_disk_inode(|disk_inode| disk_inode.read_at(offset, buf, &self.block_device))
    }
    /// Write data to current inode
    pub fn write_at(&self, offset: usize, buf: &[u8]) -> usize {
        let mut fs = self.fs.lock();
        let size = self.modify_disk_inode(|disk_inode| {
            self.increase_size((offset + buf.len()) as u32, disk_inode, &mut fs);
            disk_inode.write_at(offset, buf, &self.block_device)
        });
        block_cache_sync_all();
        size
    }
    /// Clear the data in current inode
    pub fn clear(&self) {
        let mut fs = self.fs.lock();
        self.modify_disk_inode(|disk_inode| {
            let size = disk_inode.size;
            let data_blocks_dealloc = disk_inode.clear_size(&self.block_device);
            assert!(data_blocks_dealloc.len() == DiskInode::total_blocks(size) as usize);
            for data_block in data_blocks_dealloc.into_iter() {
                fs.dealloc_data(data_block);
            }
        });
        block_cache_sync_all();
    }
    /// Integer representation of this inode
    pub fn get_id(&self)-> usize {
        self._get_id_impl(&self.fs.lock())
    }
    fn _get_id_impl(&self, fs: &EasyFileSystem)-> usize {
        const DISK_INODE_SIZE: usize = core::mem::size_of::<DiskInode>();
        const INODES_PER_BLK:  usize = BLOCK_SZ / DISK_INODE_SIZE;
        let inode_begin    = fs.get_inode_start_block_id();
        let rel_block_id = self.block_id - inode_begin as usize;
        let ret = rel_block_id * INODES_PER_BLK + self.block_offset / DISK_INODE_SIZE;
        ret
    }
    /// Get inode type: File or Directory
    pub fn is_dir_file(&self)-> (bool, bool) {
        self.read_disk_inode(|di| {
            (di.is_dir(), di.is_file())
        })
    }
    /// Get reference count
    pub fn get_ref_count(&self)-> u32 {
        self.read_disk_inode(|di| {
            di.get_ref_count() as u32
        })
    }
}
