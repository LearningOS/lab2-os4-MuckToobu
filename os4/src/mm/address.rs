
use core::fmt::{self, Debug, Formatter};
use crate::config::{PAGE_SIZE, PAGE_SIZE_BITS};
use super::PageTableEntry;

/// 物理地址最大位数
const PA_WIDTH_SV39: usize = 56;
/// 物理页号的最大位数
const PPN_WIDTH_SV39: usize = PA_WIDTH_SV39 - PAGE_SIZE_BITS;

        #[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct VirtAddr(pub usize);
        #[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct VirtPageNum(pub usize);
        #[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct PhysAddr(pub usize);
        #[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct PhysPageNum(pub usize);

pub type VPNRange = SimpleRange<VirtPageNum>;

        #[derive(Copy, Clone)]
pub struct SimpleRange<T>
    where T: StepByOne + Copy + PartialEq + PartialOrd + Debug,
{ l: T, r: T }
        #[derive(Debug)]
pub struct SimpleRangeIterator<T>
    where T: StepByOne + Copy + PartialEq + PartialOrd + Debug,
{ current: T, end: T }

pub trait StepByOne {
    fn step(&mut self);
}


//---------------------impl structs-----------------------//
impl VirtAddr {
    /// 页内偏移
    pub fn page_offset(&self) -> usize {self.0 & (PAGE_SIZE - 1)}

    pub fn aligned(&self) -> bool {(self.0 & (PAGE_SIZE - 1)) == 0}

    /// 向下取整为页号
    pub fn floor(&self) -> VirtPageNum {
        VirtPageNum(self.0 >> PAGE_SIZE_BITS)
    }
    /// 向上取整为页号
    pub fn ceil(&self) -> VirtPageNum {
        VirtPageNum((self.0 - 1 + PAGE_SIZE) >> PAGE_SIZE_BITS)
    }
}
impl VirtPageNum {
    /// 返回三级虚拟页号[一级, 二级, 三级]
    pub fn indexes(&self) -> [usize; 3] {
        let mut vpn = self.0;
        let mut idx = [0usize; 3];
        for i in (0..3) .rev() {
            idx[i] = vpn & 511;
            vpn >>= 9;
        }
        idx
    }
}
impl PhysAddr {
    /// 页内偏移
    pub fn page_offset(&self) -> usize {self.0 & (PAGE_SIZE - 1)}

    pub fn aligned(&self) -> bool {(self.0 & (PAGE_SIZE - 1)) == 0}

    /// 向下取整为页号
    pub fn floor(&self) -> PhysPageNum {
        PhysPageNum(self.0 >> PAGE_SIZE_BITS)
    }
    /// 向上取整为页号
    /// f000 -> f, f001 -> 10
    pub fn ceil(&self) -> PhysPageNum {
        PhysPageNum((self.0 - 1 + PAGE_SIZE) >> PAGE_SIZE_BITS)
    }
}
impl PhysPageNum {
    pub fn get_bytes_array(&self) -> &'static mut [u8; PAGE_SIZE] {
        unsafe {
            &mut *((self.0 << PAGE_SIZE_BITS) as *mut _)
        }
    }
    pub fn get_pte_array(&self) -> &'static mut [PageTableEntry; PAGE_SIZE / core::mem::size_of::<usize>()] {
        unsafe {
            &mut *((self.0 << PAGE_SIZE_BITS) as *mut _)
        }
    }
    pub fn get_mut<T>(&self) -> &'static mut T {
        let pa: PhysAddr = (*self).into();
        unsafe {(pa.0 as *mut T).as_mut().unwrap()}
    }
}
impl<T> SimpleRange<T>
    where T: StepByOne + Copy + PartialEq + PartialOrd + Debug,
{
    pub fn new(start: T, end: T) -> Self {
        assert!(start <= end, "start {:?} > end {:?}!", start, end);
        SimpleRange { l: start, r: end }
    }
    pub fn get_start(&self) -> T { self.l }
    pub fn get_end(&self) -> T { self.r }
    pub fn include(&self, vr: Self) -> bool { 
        ((self.l <= vr.l) & (vr.l < self.r)) 
        | ((vr.l <= self.l) & (self.l < vr.r)) }
}
impl<T> SimpleRangeIterator<T>
    where T: StepByOne + Copy + PartialEq + PartialOrd + Debug,
{
    fn new(l: T, r: T) -> Self {
        SimpleRangeIterator { current: l, end: r }
    }
}

//-------------------Impl Traits----------------------//
impl StepByOne for VirtPageNum {
    fn step(&mut self) { self.0 += 1}
}

//------------------Iterator-------------------------//

impl<T> IntoIterator for SimpleRange<T>
    where T: StepByOne + Copy + PartialEq + PartialOrd + Debug,
{
    type Item = T;
    type IntoIter = SimpleRangeIterator<T>;
    fn into_iter(self) -> Self::IntoIter {
        SimpleRangeIterator::new(self.l, self.r)
    }
}
impl<T> Iterator for SimpleRangeIterator<T>
    where T: StepByOne + Copy + PartialEq + PartialOrd + Debug,
{
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        if self.current == self.end {
            None
        } else {
            let t = self.current;
            self.current.step();
            Some(t)
        }
    }
}

//--------------------From/Into---------------------//
impl From<PhysPageNum> for PhysAddr {
    fn from(v: PhysPageNum) -> Self {
        PhysAddr(v.0 << PAGE_SIZE_BITS)
    }
}
impl From<PhysAddr> for PhysPageNum {
    fn from(v: PhysAddr) -> Self {
        assert_eq!(v.page_offset(), 0);
        PhysPageNum(v.0 >> PAGE_SIZE_BITS)
    }
}
impl From<VirtPageNum> for VirtAddr {
    fn from(v: VirtPageNum) -> Self {
        VirtAddr(v.0 << PAGE_SIZE_BITS)
    }
}
impl From<VirtAddr> for VirtPageNum {
    fn from(v: VirtAddr) -> Self {
        assert_eq!(v.page_offset(), 0);
        VirtPageNum(v.0 >> PAGE_SIZE_BITS)
    }
}



//------------------impl Debug-------------------//
impl Debug for VirtAddr {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("VA:{:#x}", self.0))
    }
}
impl Debug for VirtPageNum {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("VPN:{:#x}", self.0))
    }
}
impl Debug for PhysAddr {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("PA:{:#x}", self.0))
    }
}
impl Debug for PhysPageNum {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("PPN:{:#x}", self.0))
    }
}
impl<T> Debug for SimpleRange<T>
    where T: StepByOne + Copy + PartialEq + PartialOrd + Debug,
{
    fn fmt (&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("SimpleRange[{:?}, {:?}]", self.l, self.r))
    }
}