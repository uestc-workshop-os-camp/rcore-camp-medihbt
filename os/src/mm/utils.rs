//! Memory management utilities including kernel-user memory interactions.
//! BY Medi H.B.T.

use crate::{mm, task};


/// Copy N bytes to kernel space from user space.
pub unsafe fn copy_from_user(kernel_src: &mut [u8], user_src: *const u8, len: usize)
{
    assert!(kernel_src.len() >= len);
    assert!(user_src != core::ptr::null());
    let cur_mm_token = task::current_user_token();
    let mapped_range = mm::translated_byte_buffer(cur_mm_token, user_src, len);
    let mut kbegin: usize = 0;
    for phys in mapped_range {
        let plen = phys.len();
        let kend = kbegin + plen;
        kernel_src[kbegin..kend].copy_from_slice(phys);
        kbegin = kend;
    }
}

/// Copy a POD object from user space to kernel space.
/// You CANNOT use this function to copy anything handling resources, e.g. Vec<int>.
pub fn copy_obj_from_user<DataT: Sized+Copy>(kobject: &mut DataT, user_src: *const DataT)
{
    let len  = core::mem::size_of::<DataT>();
    let kptr = kobject as *mut DataT as *mut u8;
    unsafe {
        copy_from_user(
            core::slice::from_raw_parts_mut(kptr, len),
            user_src as *const u8, len
        );
    };
}

/// Copy N bytes to user space from kernel space.
pub unsafe fn copy_to_user(user_dst: *mut u8, len: usize, kernel_src: &[u8])
{
    assert!(user_dst != core::ptr::null_mut());
    let cur_mm_token = task::current_user_token();
    let mapped_range = mm::translated_byte_buffer(cur_mm_token, user_dst, len);
    let mut kbegin: usize = 0;
    let ksafe_end:  usize = kernel_src.len().min(len);
    for phys in mapped_range {
        let kend = ksafe_end.min(kbegin + phys.len());
        phys.copy_from_slice(&kernel_src[kbegin..kend]);
        if kend >= ksafe_end {
            break;
        }
        kbegin += phys.len();
    }
}

/// Copy a POD object from kernel space to user space.
/// You CANNOT use this function to copy anything handling resources, e.g. Vec<int>.
pub unsafe fn copy_obj_to_user<DataT: Sized>(user_dst: *mut DataT, kobject: &DataT)
{
    let len = core::mem::size_of::<DataT>();
    let kptr = kobject as *const DataT as *const u8;
    let uptr = user_dst as *mut u8;
    unsafe {
        copy_to_user(uptr, len, core::slice::from_raw_parts(kptr, len));
    }
}

/// Utilities for memory mapping(mmap & munmap syscalls)
///
/// BY Medi.H.B.T.
pub mod mmap_handle {
    use crate::{config::PAGE_SIZE, mm::{address::VPNRange, MapPermission, MemorySet, VirtAddr, VirtPageNum}, task::update_current_tcb};

    /// Check if an address or a length is page-aligned.
    fn page_aligned(addr: usize)-> bool {
        (addr & 0x0000_0FFF) == 0
    }
    fn uprot_to_permission(prot: usize)-> Option<MapPermission> {
        if prot >= 8 || (prot & 0x7) == 0 {
            return None;
        }
        trace!("Prot value 0b{:04b}", prot);
        MapPermission::from_bits((prot << 1) as u8)
            .map(|p| { p | MapPermission::U })
    }
    fn _check_vaddr_if_mapped(mset: &MemorySet, start: usize, end: usize)-> bool {
        let vstart: VirtAddr = start.into();
        let vend:   VirtAddr = end.into();
        let vpns = VPNRange::new(
            VirtPageNum::from(vstart), VirtPageNum::from(vend)
        );
        for vp in vpns {
            if let Some(vp) = mset.translate(vp) {
                if vp.is_valid() { return true; }
            }
        }
        false
    }
    fn make_page_aligned(addr: usize)-> usize {
        if page_aligned(addr) {
            addr
        } else {
            (addr | 0x0000_0FFF) + 1
        }
    }

    /// Handle memory mapping
    pub fn do_mmap(start: usize, len: usize, prot: usize)-> isize
    {
        /* Check addresses to ensure they're page-aligned. */
        if !page_aligned(start) {
            warn!("Address(start) 0x{:016x} not page-aligned", start);
            return -1;
        }
        /* Check protection bits. */
        let prot = if let Some(prot) = uprot_to_permission(prot) {
            prot
        } else {
            warn!("Invalid permission value 0b{:04b}", prot);
            return -1;
        };

        let len = make_page_aligned(len);

        /* Access TCB: we'll update memory sets so that memory mapping infomation
         * will be registered   */
        update_current_tcb(&mut |_tcb, tcbi| {
            let mset = &mut tcbi.memory_set;
            if _check_vaddr_if_mapped(mset, start, start + len) {
                warn!("Virtual address {:016x}..{:016x} mapped", start, start + len);
                return -1;
            }
            mset.insert_framed_area(
                VirtAddr::from(start),
                VirtAddr::from(start + len),
                prot);
            return 0;
        })
    }

    /// Handle memory unmapping
    pub fn do_munmap(start: usize, len: usize)-> isize {
        /* Check addresses to ensure they're page-aligned. */
        if !page_aligned(start) {
            warn!("Address(start) 0x{:016x} not page-aligned", start);
            return -1;
        }

        let plen   = make_page_aligned(len);
        let npages = plen / PAGE_SIZE;
        let pbegin = VirtPageNum::from(VirtAddr(start));

        info!("Unmap len {:08x}, {:08x} pages", len, npages);

        /* Access TCB: we'll update memory sets so that memory mapping infomation
         * will be registered or unregistered  */
        update_current_tcb(&mut |_tcb, tcbi| {
            if tcbi.memory_set.unmap_range(pbegin, npages) { 0 } else { -1 }
        })
    }
}