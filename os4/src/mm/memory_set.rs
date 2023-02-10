use alloc::rc::Rc;
use lazy_static::lazy_static;
use xmas_elf;
use riscv::register::satp;
use crate::config::{TRAMPOLINE, PAGE_SIZE, MEMORY_END,
    USER_STACK_SIZE, TRAP_CONTEXT};
use crate::sync::UPSafeCell;
use super::{PhysPageNum, VirtAddr, PageTable, VPNRange, 
    VirtPageNum, FrameTracker, PTEFlags, StepByOne,
    PageTableEntry, PhysAddr, frame_alloc};
use alloc::vec::Vec;
use alloc::collections::BTreeMap;

bitflags! {
    pub struct MapPermission: u8 {
        const R = 1 << 1;
        const W = 1 << 2;
        const X = 1 << 3;
        const U = 1 << 4;
    }
}
    #[derive(Copy, Clone, PartialEq, Debug)]
pub enum MapType {
    Identical,
    Framed,
}

pub struct MapArea {
    vpn_range: VPNRange,
    /// 所有数据所占的FramTracker，不包括页表项。
    data_frames: BTreeMap<VirtPageNum, FrameTracker>,
    map_type: MapType,
    map_perm: MapPermission,
}
pub struct MemorySet {
    page_table: PageTable,
    areas: Vec<MapArea>,
}

lazy_static! {
    pub static ref KERNEL_SPACE: UPSafeCell<MemorySet> = 
        unsafe{ UPSafeCell::new(MemorySet::new_kernel()) };
}

//-----------------------impl structs------------------------//
impl MapArea {
    pub fn new(
        start_va: VirtAddr,
        end_va: VirtAddr,
        map_type: MapType,
        map_perm: MapPermission,
    ) -> Self {
        let start_vpn = start_va.floor();
        let end_vpn = end_va.ceil();
        MapArea {
            vpn_range: VPNRange::new(start_vpn, end_vpn),
            data_frames: BTreeMap::new(),
            map_type,
            map_perm,
        }
    }
    pub fn include(&self, vr: VPNRange) -> bool {
        self.vpn_range.include(vr)
    }
    pub fn match_r(&self, vr: VPNRange) -> bool {
        (self.vpn_range.get_start() == vr.get_start())
        & (self.vpn_range.get_end() == vr.get_end())
    }
    pub fn map_one(&mut self, page_table: &mut PageTable, vpn: VirtPageNum) {
        let ppn: PhysPageNum;
        match self.map_type {
            MapType::Identical => ppn = PhysPageNum(vpn.0),
            MapType::Framed => {
                let frame = frame_alloc().unwrap();
                ppn = frame.ppn;
                self.data_frames.insert(vpn, frame);
            }
        }
        let pte_flags = PTEFlags::from_bits(self.map_perm.bits).unwrap();
        page_table.map(vpn, ppn, pte_flags);
    }
    pub fn unmap_one(&mut self,  page_table: &mut PageTable, vpn: VirtPageNum) {
        match self.map_type {
            MapType::Framed => {
                self.data_frames.remove(&vpn);
            }
            _ => {}
        }
        page_table.unmap(vpn);
    }
    /// 将所有vpn_range中所有vpn映射到物理页面
    pub fn map(&mut self, page_table: &mut PageTable) {
        for vpn in self.vpn_range {
            self.map_one(page_table, vpn);
        }
    }
    /// 将所有vpn_range中所有vpn映射到物理页面
    pub fn unmap(&mut self, page_table: &mut PageTable) {
        for vpn in self.vpn_range {
            self.unmap_one(page_table, vpn);
        }
    }
    /// data: must be start-aligned
    /// assume that all frames were cleared before
    pub fn copy_data(&mut self, page_table: &mut PageTable, data: &[u8]) {
        assert_eq!(self.map_type, MapType::Framed);
        let mut start: usize = 0;
        let mut current_vpn = self.vpn_range.get_start();
        let len = data.len();
        loop {
            let src = &data[start..len.min(start + PAGE_SIZE)];
            let dst = &mut page_table
                .translate(current_vpn).unwrap()
                .ppn()
                .get_bytes_array()[..src.len()];
            dst.copy_from_slice(src);
            start += PAGE_SIZE;
            if start >= len {
                break;
            }
            current_vpn.step();
        }
    } 
}
impl MemorySet {
    pub fn new_bare() -> Self {
        MemorySet {
            page_table: PageTable::new(),
            areas: Vec::new(),
        }
    }
    fn map_trampoline(&mut self) {
        self.page_table.map(
            VirtAddr(TRAMPOLINE).floor(),
            PhysAddr(strampoline as usize).floor(),
            PTEFlags::R | PTEFlags::X,
        )
    }
    fn push(&mut self, mut map_area: MapArea, data: Option<&[u8]>) {
        map_area.map(&mut self.page_table);
        if let Some(data) = data {
            map_area.copy_data(&mut self.page_table, data);
        }
        self.areas.push(map_area);
    }
    pub fn insert_framed_area(
        &mut self,
        start_va: VirtAddr,
        end_va: VirtAddr,
        pernission: MapPermission,
    ) {
        self.push(
            MapArea::new(start_va, end_va, MapType::Framed,
                pernission),
            None,
        );
    }
    pub fn active(&self) {
        let satp = self.page_table.token();
        unsafe {
            satp::write(satp);
            core::arch::asm!("sfence.vma");
        }
    }
    pub fn translate(&self, vpn: VirtPageNum) -> Option<PageTableEntry> {
        self.page_table.translate(vpn)
    }
    pub fn translate_addr_unchecked(&self, va: VirtAddr) -> Option<PhysAddr> {
        let offset = va.page_offset();
        let vpn = va.floor();
        match self.translate(vpn) {
            Some(pte) => {
                // 此处未检查内存合法性
                let pa: PhysAddr = pte.ppn().into();
                Some(PhysAddr(pa.0 + offset))
            }
            None => None,
        }
    }
    fn include(&self, vr: VPNRange) -> bool {
        for i in self.areas.iter() {
            if i.include(vr) { 
                println!("maparea: {:?}", i.vpn_range);
                println!("vr {:?}", vr);
                return true }
        }
        false
    }
    pub fn map_create(&mut self, start: VirtAddr, len: usize, port: MapPermission) -> isize {
        let vr = VPNRange::new(
                start.floor(),
                VirtAddr(start.0 + len).ceil()
        );
        if self.include(vr) {return -1}
        self.push(
            MapArea::new(
                start,
                VirtAddr(start.0 + len),
                MapType::Framed,
                port,
            ), None,
        );
        0
    }
    pub fn munmap(&mut self, start: VirtAddr, len: usize) -> isize {
        let vr = VPNRange::new(
                start.floor(),
                VirtAddr(start.0 + len).ceil()
        );
        let mut res = -1;
        let mut idx = 0;
        for ma in self.areas.iter_mut().enumerate() {
            if ma.1.match_r(vr) {
                ma.1.unmap(&mut self.page_table);
                idx = ma.0;
                res = 0;  
            }
        }
        self.areas.remove(idx);
        res
    }
    pub fn token(&self) -> usize {
        self.page_table.token()
    }
    pub fn new_kernel() -> Self {
        let mut memory_set = Self::new_bare();
        memory_set.map_trampoline();
        info!(".text [{:#x}, {:#x})", stext as usize, etext as usize);
        info!(".rodata [{:#x}, {:#x})", srodata as usize, erodata as usize);
        info!(".data [{:#x}, {:#x})", sdata as usize, edata as usize);
        info!(".bss [{:#x}, {:#x})", sbss_with_stack as usize, ebss as usize);
        
        println!("mapping .text section");
        memory_set.push(
            MapArea::new(
                VirtAddr(stext as usize),
                VirtAddr(etext as usize),
                MapType::Identical,
                MapPermission::R | MapPermission::X,
            ), None,
        );
        println!("mapping .rodata memory");
        memory_set.push(
            MapArea::new(
                VirtAddr(srodata as usize),
                VirtAddr(erodata as usize),
                MapType::Identical,
                MapPermission::R,
            ), None,
        );
        println!("mapping .data section");
        memory_set.push(
            MapArea::new(
                VirtAddr(sdata as usize),
                VirtAddr(edata as usize),
                MapType::Identical,
                MapPermission::R | MapPermission::W,
            ), None,
        );
        println!("mapping .bss section");
        memory_set.push(
            MapArea::new(
                VirtAddr(sbss_with_stack as usize),
                VirtAddr(ebss as usize),
                MapType::Identical,
                MapPermission::R | MapPermission::W,
            ), None,
        );
        println!("mapping physical memory");
        memory_set.push(
            MapArea::new(
                VirtAddr(ekernel as usize).into(),
                VirtAddr(MEMORY_END),
                MapType::Identical,
                MapPermission::R | MapPermission::W,
            ), None
        );
        memory_set
    }
    /// Include sections in elf and trampoline and TrapContext and user stack,
    /// alos returns user_sp and entry point.
    pub fn from_elf(elf_data: &[u8]) -> (Self, VirtAddr, VirtAddr) {
        let mut memory_set = MemorySet::new_bare();
        memory_set.map_trampoline();
        // map program headers of elf, with U flag
        let elf = xmas_elf::ElfFile::new(elf_data).unwrap();
        let elf_header = elf.header;
        let magic = elf_header.pt1.magic;
        assert_eq!(magic, [0x7f, 0x45, 0x4c, 0x46], "invalid elf");
        let ph_count = elf_header.pt2.ph_count();
        let mut max_end_vpn = VirtPageNum(0);
        for i in 0..ph_count {
            let ph = elf.program_header(i).unwrap();
            if ph.get_type().unwrap() == xmas_elf::program::Type::Load {
                let start_va = VirtAddr(ph.virtual_addr() as usize);
                let end_va = VirtAddr((ph.virtual_addr() + ph.mem_size()) as usize);
                let mut map_perm = MapPermission::U;
                let ph_flags = ph.flags();
                if ph_flags.is_read() {
                    map_perm |= MapPermission::R;
                }
                if ph_flags.is_write() {
                    map_perm |= MapPermission::W;
                }
                if ph_flags.is_execute() {
                    map_perm |= MapPermission::X;
                }
                let map_area = MapArea::new(
                    start_va, end_va,
                    MapType::Framed, map_perm);
                max_end_vpn = map_area.vpn_range.get_end();
                memory_set.push(
                    map_area,
                    Some(&elf.input[ph.offset() as usize ..(ph.offset() + ph.file_size()) as usize]),
                );
            }
        }
        // map user stack with U flags
        let max_end_va: VirtAddr = max_end_vpn.into();
        let mut user_stack_bottom: usize = max_end_va.0;
        // guard page
        user_stack_bottom += PAGE_SIZE;
        let user_stack_top = user_stack_bottom + USER_STACK_SIZE;
        memory_set.push(
            MapArea::new(
                VirtAddr(user_stack_bottom),
                VirtAddr(user_stack_top),
                MapType::Framed,
                MapPermission::R | MapPermission::W | MapPermission::U,
            ), None,
        );
        // map TrapContext
        memory_set.push(
            MapArea::new(
                VirtAddr(TRAP_CONTEXT),
                VirtAddr(TRAMPOLINE),
                MapType::Framed,
                MapPermission::R | MapPermission::W,
            ), None,
        );
        (
            memory_set,
            VirtAddr(user_stack_top),
            VirtAddr(elf.header.pt2.entry_point() as usize),
        )
    }
}

extern "C" {
    fn stext();
    fn etext();
    fn srodata();
    fn erodata();
    fn sdata();
    fn edata();
    fn sbss_with_stack();
    fn ebss();
    fn ekernel();
    fn strampoline();
}

#[allow(unused)]
pub fn remap_test() {
    let mut kernel_space = KERNEL_SPACE.exclusive_access();
    let mid_text = VirtAddr((stext as usize + etext as usize)/2);
    let mid_rodata = VirtAddr((srodata as usize + erodata as usize)/2);
    let mid_data = VirtAddr((sdata as usize + edata as usize)/2);
    assert!(!kernel_space
        .page_table
        .translate(mid_text.floor())
        .unwrap()
        .writable());
    assert!(!kernel_space
        .page_table
        .translate(mid_rodata.floor())
        .unwrap()
        .writable());
    assert!(!kernel_space
        .page_table
        .translate(mid_data.floor())
        .unwrap()
        .executable());
}