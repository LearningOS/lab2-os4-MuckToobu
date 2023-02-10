
use core::fmt::{self, Debug, Formatter};

use bitflags::bitflags;
use alloc::vec;
use alloc::vec::Vec;
use super::{frame_alloc, FrameTracker,
    PhysPageNum, StepByOne, VirtAddr, VirtPageNum};


//--------------------structs----------------------//
    #[repr(C)]
    #[derive(Copy, Clone)]
pub struct PageTableEntry(pub usize);

pub struct PageTable{
    root_ppn: PhysPageNum,
    /// 页表节点所占用的FrameTracker
    frames: Vec<FrameTracker>,
}

bitflags! {
    pub struct PTEFlags: u8 {
        const V = 1 << 0;
        const R = 1 << 1;
        const W = 1 << 2;
        const X = 1 << 3;
        const U = 1 << 4;
        const G = 1 << 5;
        const A = 1 << 6;
        const D = 1 << 7;
    }
}

//----------------------functions------------------------//

/// 获取连续的地址空间
pub fn translated_byte_buffer(token: usize, ptr: *const u8, len: usize) -> Vec<&'static mut [u8]> {
    let page_table = PageTable::from_token(token);
    let mut start = ptr as usize;
    let end = start + len;
    let mut v = Vec::new();
    while start < end {
        let start_va = VirtAddr(start);
        let mut vpn = start_va.floor();
        let ppn = page_table.translate(vpn).unwrap().ppn();
        vpn.step();
        let mut end_va: VirtAddr = vpn.into();
        end_va = end_va.min(VirtAddr(end));
        if end_va.page_offset() == 0 {
            v.push(&mut ppn.get_bytes_array()[start_va.page_offset()..]);
        } else {
            v.push(&mut ppn.get_bytes_array()[start_va.page_offset()..end_va.page_offset()]);
        }
        start = end_va.0;
    }
    v
}
//-----------------impl structs--------------------//
impl PageTableEntry {
    fn new(ppn: PhysPageNum, flags: PTEFlags) -> Self {
        PageTableEntry(ppn.0<<10 | flags.bits as usize)
    }
    pub fn empty() -> Self {
        PageTableEntry(0)
    }
    pub fn ppn(&self) -> PhysPageNum {
        PhysPageNum((self.0 >> 10) & ((1 << 44) - 1))
    }
    pub fn flags(&self) -> PTEFlags {
        PTEFlags::from_bits(self.0 as u8).unwrap() 
    }
    pub fn is_valid(&self) -> bool {
        (self.flags() & PTEFlags::V) != PTEFlags::empty()
    }
    pub fn readable(&self) -> bool {
        (self.flags() & PTEFlags::R) != PTEFlags::empty()
    }
    pub fn writable(&self) -> bool {
        (self.flags() & PTEFlags::W) != PTEFlags::empty()
    }
    pub fn executable(&self) -> bool {
        (self.flags() & PTEFlags::X) != PTEFlags::empty()
    }
}
impl PageTable {
    pub fn new() -> Self {
        let frame = frame_alloc().unwrap();
        PageTable {
            root_ppn: frame.ppn,
            frames: vec![frame],
        }
    }

    fn find_pte_create(&mut self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let mut idxs = vpn.indexes();
        let mut ppn = self.root_ppn;
        let mut result: Option<&mut PageTableEntry> = None;
        for (i, idx) in idxs.iter_mut().enumerate() {
            let pte = &mut ppn.get_pte_array()[*idx];
            // 此处一定是`==`，如果是`>`，会导致申请一个第四级的页表
            if i == 2 {
                // 此处返回了一个未初始化的页表entry
                result = Some(pte);
                break;
            }
            if !pte.is_valid() {
                // 此处申请时会将FrameTracker.ppn页表全部置零。
                let frame = frame_alloc().unwrap();
                *pte = PageTableEntry::new(frame.ppn, PTEFlags::V);
                self.frames.push(frame);
            }
            ppn = pte.ppn();
        }
        result
    } 
    fn find_pte(&self, vpn: VirtPageNum) -> Option<&PageTableEntry> {
        let idxs = vpn.indexes();
        let mut ppn = self.root_ppn;
        let mut result: Option<&PageTableEntry> = None;
        for (i, idx) in idxs.iter().enumerate() {
            let pte = &mut ppn.get_pte_array()[*idx];
            if i == 2 {
                result = Some(pte);
                break;
            }
            if !pte.is_valid() {
                return None;
            }
            ppn = pte.ppn();
        }
        result
    }
    pub fn map(&mut self, vpn: VirtPageNum, ppn: PhysPageNum, flags: PTEFlags) {
        let pte = self.find_pte_create(vpn).unwrap();
        assert!(!pte.is_valid(), "vpn {:?} is mapped before mapping", vpn);
        *pte = PageTableEntry::new(ppn, flags | PTEFlags::V);
    }
    pub fn unmap(&mut self, vpn: VirtPageNum) {
        let pte = self.find_pte_create(vpn).unwrap();
        assert!(pte.is_valid(), "vpn {:?} is invalid before unmapping", vpn);
        // 这里为什么不需要deallocate frame？
        // 因为这里是页表，不包含分配下去的页面。
        *pte = PageTableEntry::empty();
    }
    pub fn translate(&self, vpn: VirtPageNum) -> Option<PageTableEntry> {
        self.find_pte(vpn).copied()
    }
    // 从satp获得PageTableEntry
    pub fn from_token(satp: usize) -> Self {
        PageTable {
            root_ppn: PhysPageNum(satp & ((1 << 44) - 1 )),
            frames: Vec::new(),
        }
    }
    // 返回所用页表为SV39
    pub fn token(&self) -> usize {
        8usize <<60 | self.root_ppn.0
    }

}

//---------------------------impl Debug--------------------------------//
impl Debug for PageTableEntry {
    fn fmt(&self, f:&mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("PTE:{:#x}", self.0))
    }
}
