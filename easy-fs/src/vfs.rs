use core::mem::size_of;

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
    // // CH6 ADDED
    // pub nlink: u32,
    // // CH6 ADDED
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
            // // CH6 ADDED
            // nlink: 1,
            // // CH6 ADDED
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

        // CH6 ADDED
        assert!(fs.get_inode_id(new_inode_block_id, new_inode_block_offset) == new_inode_id);
        // CH6 ADDED

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


    // CH6 ADDED
    /// Create link
    pub fn state(&self) -> (u64, bool, u32){
    // pub fn state(&self) -> (u64, DiskInodeType, u32){
        let fs = self.fs.lock();
        //查询自身inode id
        // let mut self_inode_id: isize = -1;
        // self.read_disk_inode(|disk_inode| {
        //     self.find_inode_id(old_path, disk_inode).map(|inode_id| {
        //         self_inode_id = inode_id as isize
        //     })
        // });
        // assert!(self_inode_id != -1);

        let (nlink, _type) = self.read_disk_inode(|disk_inode| {
            // (disk_inode.nlink, if disk_inode.is_dir() {DiskInodeType::Directory} else {DiskInodeType::File})
            (disk_inode.nlink, disk_inode.is_dir())
        });


        let inode_id = fs.get_inode_id(self.block_id as u32, self.block_offset);


        // /// inode number
        // pub ino: u64,
        // /// file type and mode
        // pub mode: StatMode,
        // /// number of hard links
        // pub nlink: u32,

        (inode_id as u64, _type, nlink)




    }


    /// Create link
    pub fn link(&self, old_path: &str, new_path: &str) -> isize{
        let mut fs = self.fs.lock();
        let mut target_inode_id: isize = -1;
        // self.read_disk_inode(|disk_inode| {
        //     self.find_inode_id(old_path, disk_inode).map(|inode_id| {
        //         target_inode_id = inode_id as isize
        //     })
        // });

        let old_inode = self.read_disk_inode(|disk_inode| {
            self.find_inode_id(old_path, disk_inode).map(|inode_id| {
                target_inode_id = inode_id as isize;
                let (block_id, block_offset) = fs.get_disk_inode_pos(inode_id);
                Arc::new(Self::new(
                    block_id,
                    block_offset,
                    self.fs.clone(),
                    self.block_device.clone(),
                ))
            })
        }).unwrap();

        assert!(target_inode_id != -1);

        

        // let old_inode = self.find(old_path).unwrap();
        old_inode.modify_disk_inode(|disk_inode| {
            disk_inode.nlink += 1
        });

        // self.modify_disk_inode(|disk_inode| {
        //     // disk_inode.nlink += 1
        //     disk_inode.nlink = 5;
        //     // self.find_inode_id(name, disk_inode).map(|inode_id| {
        //     //     let (block_id, block_offset) = fs.get_disk_inode_pos(inode_id);
        //     //     Arc::new(Self::new(
        //     //         block_id,
        //     //         block_offset,
        //     //         self.fs.clone(),
        //     //         self.block_device.clone(),
        //     //     ))
        //     // })
        // });
        // block_cache_sync_all();

        self.modify_disk_inode(|root_inode| {
            // append file in the dirent
            let file_count = (root_inode.size as usize) / DIRENT_SZ;
            let new_size = (file_count + 1) * DIRENT_SZ;
            // increase size
            self.increase_size(new_size as u32, root_inode, &mut fs);
            // write dirent
            let dirent = DirEntry::new(new_path, target_inode_id as u32);
            root_inode.write_at(
                file_count * DIRENT_SZ,
                dirent.as_bytes(),
                &self.block_device,
            );
        });
        block_cache_sync_all();
        1
    }


    /// Delte link
    pub fn unlink(&self, name: &str) -> isize{
        let fs = self.fs.lock();
        // let mut target_inode_id: isize = -1;
        // self.read_disk_inode(|disk_inode| {
        //     self.find_inode_id(old_path, disk_inode).map(|inode_id| {
        //         target_inode_id = inode_id as isize
        //     })
        // });

        let old_inode = self.read_disk_inode(|disk_inode| {
            self.find_inode_id(name, disk_inode).map(|inode_id| {
                // target_inode_id = inode_id as isize;
                let (block_id, block_offset) = fs.get_disk_inode_pos(inode_id);
                Arc::new(Self::new(
                    block_id,
                    block_offset,
                    self.fs.clone(),
                    self.block_device.clone(),
                ))
            })
        }).unwrap();

        // assert!(target_inode_id != -1);

        

        // let old_inode = self.find(old_path).unwrap();
        old_inode.modify_disk_inode(|disk_inode| {
            assert!(disk_inode.nlink > 0);
            disk_inode.nlink -= 1;
            if disk_inode.nlink == 0 {
                // TODO DEL
            }
        });

        // self.modify_disk_inode(|disk_inode| {
        //     // disk_inode.nlink += 1
        //     disk_inode.nlink = 5;
        //     // self.find_inode_id(name, disk_inode).map(|inode_id| {
        //     //     let (block_id, block_offset) = fs.get_disk_inode_pos(inode_id);
        //     //     Arc::new(Self::new(
        //     //         block_id,
        //     //         block_offset,
        //     //         self.fs.clone(),
        //     //         self.block_device.clone(),
        //     //     ))
        //     // })
        // });
        // block_cache_sync_all();


        self.modify_disk_inode(|disk_inode| {
            let file_count = (disk_inode.size as usize) / DIRENT_SZ;
            // let mut v: Vec<String> = Vec::new();
            for i in 0..file_count {
                let mut dirent = DirEntry::empty();
                assert_eq!(
                    disk_inode.read_at(i * DIRENT_SZ, dirent.as_bytes_mut(), &self.block_device,),
                    DIRENT_SZ,
                );
                // v.push(String::from(dirent.name()));
                if dirent.name() == name{
                    let new_dirent = DirEntry::new("_unlink", dirent.inode_id());
                    assert!(core::mem::size_of::<DirEntry>() == 32);
                    disk_inode.write_at(i * DIRENT_SZ, &new_dirent.as_bytes(), &self.block_device);

                    // BAD APPLE HERE
                    // TODOOOOO
                    // TODOOOOO
                    // TODOOOOO
                    // TODOOOOO
                    // TODOOOOO
                    // TODOOOOO
                    // block_cache_sync_all();
                    return 0;
                }
            }
            assert!(false);
            -1
        })

        // self.modify_disk_inode(|root_inode| {
        //     // append file in the dirent
        //     let file_count = (root_inode.size as usize) / DIRENT_SZ;
        //     let new_size = (file_count + 1) * DIRENT_SZ;
        //     // increase size
        //     self.increase_size(new_size as u32, root_inode, &mut fs);
        //     // write dirent
        //     let dirent = DirEntry::new(new_path, target_inode_id as u32);
        //     root_inode.write_at(
        //         file_count * DIRENT_SZ,
        //         dirent.as_bytes(),
        //         &self.block_device,
        //     );
        // });
        // block_cache_sync_all();
        // 1
        // -1
    }

    

    // CH6 ADDED
}
