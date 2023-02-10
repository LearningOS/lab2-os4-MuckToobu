
use super::{PhysAddr, PhysPageNum};
use crate::config::MEMORY_END;
use crate::sync::UPSafeCell;
use alloc::vec::Vec;
use core::fmt::{self, Debug, Formatter};
use lazy_static::lazy_static;


/// 跟踪物理页帧，并保管
pub struct FrameTracker {
    pub ppn: PhysPageNum,
}
pub struct StackFrameAllocator {
    current: PhysPageNum,
    end: PhysPageNum,
    recycled: Vec<PhysPageNum>
}

lazy_static! {
    pub static ref FRAME_ALLOCATOR: UPSafeCell<StackFrameAllocator>
        = unsafe { UPSafeCell::new(StackFrameAllocator::new())};
}

pub fn init_frame_allocator() {
    extern "C" {
        fn ekernel();
    }
    FRAME_ALLOCATOR.exclusive_access().init(
        PhysAddr(ekernel as usize).ceil(),
        PhysAddr(MEMORY_END).floor(),
    )
}
pub fn frame_alloc() -> Option<FrameTracker> {
    FRAME_ALLOCATOR.exclusive_access()
        .alloc().map(FrameTracker::new)
}
pub fn frame_dealloc(ppn: PhysPageNum) {
    FRAME_ALLOCATOR.exclusive_access().dealloc(ppn);
}

//--------------------impl structs----------------------//
impl FrameTracker {
    // 此处即初始化，使得页表项不合法：PTEFlags::V置零
    fn new(ppn: PhysPageNum) -> Self {
        let bytes_array = ppn.get_bytes_array();
        for i in bytes_array {
            *i = 0;
        }
        FrameTracker { ppn }
    }
}

impl StackFrameAllocator {
    fn new() -> Self {
        StackFrameAllocator {
            current: PhysPageNum(0),
            end: PhysPageNum(0),
            recycled: Vec::new(),
        }
    }
    fn init(&mut self, l: PhysPageNum, r: PhysPageNum) {
        self.current = l;
        self.end = r;
    }
    fn alloc(&mut self) -> Option<PhysPageNum> {
        if let Some(ppn) = self.recycled.pop() {
            Some(ppn)
        } else if self.current == self.end {
            None
        } else {
            let ret = self.current;
            self.current.0 += 1;
            Some(ret)
        }
    }
    fn dealloc(&mut self, ppn: PhysPageNum) {
        if (ppn >= self.current) | self.recycled.iter().any(|v| *v == ppn ) {
            panic!("Frame {:?} has not been allocated!", ppn)
        }
        self.recycled.push(ppn);
    }
}

//------------------------impl Drop---------------------------//
impl Drop for FrameTracker {
    fn drop(&mut self) {
        frame_dealloc(self.ppn);
    }
}

//------------------------impl Debug--------------------------//
impl Debug for FrameTracker {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("FrameTracker:PPN={:#x}", self.ppn.0))
    }
}
impl Debug for StackFrameAllocator {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!(
            "FrameAllocator: current {:?}, end {:?}",
            self.current, self.end,
        ))
    }
}