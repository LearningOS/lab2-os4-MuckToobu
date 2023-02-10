mod memory_set;
mod address;
mod frame_allocator;
// 堆分配器，提供`Vec`、`Box`、`Arc`等。
mod heap_allocator;
mod page_table;

pub use address::{VirtAddr, VirtPageNum, PhysAddr,
    PhysPageNum, VPNRange, StepByOne};
pub use frame_allocator::{frame_alloc, FrameTracker};
pub use page_table::{PageTable, PageTableEntry, PTEFlags, 
    translated_byte_buffer};
pub use memory_set::{MapPermission, MapArea, MapType,
    MemorySet, KERNEL_SPACE, remap_test};


/// 初始化`heap_allocator`,`frame_allocator`,`kernel_space`
pub fn init() {
    heap_allocator::init_heap();
    frame_allocator::init_frame_allocator();
    KERNEL_SPACE.exclusive_access().active();
}