use crate::{config::MAX_SYSCALL_NUM, task::{TaskStatus, exit_current_and_run_next, 
    suspend_current_and_run_next, translate, current_syscall_info, current_start_time,
    current_map_crate, current_munmap}, timer::get_time_us, mm::MapPermission};
use crate::mm::{VirtAddr};



#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

#[derive(Clone, Copy)]
pub struct TaskInfo {
    pub status: TaskStatus,
    pub syscall_times: [u32; MAX_SYSCALL_NUM],
    pub time: usize,
}

pub fn sys_exit(exit_code: i32) -> ! {
    info!("[kernel] Application exited with code {}", exit_code);
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gaves up resources for other tasks
pub fn sys_yield() -> isize {
    suspend_current_and_run_next();
    0
}

// your job: 引入虚地址后重写 sys_get_time
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    let us = get_time_us();
    match translate(VirtAddr(ts as usize)) {
        Some(pa) => {
            unsafe { 
                let ts = pa.0 as *mut TimeVal;
                *ts = TimeVal {
                    sec: us / 1_000_000,
                    usec: us % 1_000_000,
                }
            };
            0
        }
        None => -1,
    }
}

// your job: 扩展内核以实现sys_mmap和sys_munmap
pub fn sys_mmap(start: usize, len: usize, port: usize) -> isize {
    let va = VirtAddr(start);
    if !va.aligned() {return -1};
    if !((port > 0) & (port < 8)) {return -1;}
    let port = MapPermission::from_bits(((port << 1) + 16) as u8).unwrap();
    current_map_crate(va, len, port)
}

pub fn sys_munmap(start: usize, len: usize) -> isize {
    let va = VirtAddr(start);
    if !va.aligned() {return -1};
    current_munmap(VirtAddr(start), len)
}

//your job: 引入虚地址后重写 sys_task_info 
pub fn sys_task_info(ti: *mut TaskInfo) -> isize {
    match translate(VirtAddr(ti as usize)) {
        Some(pa) => {
            unsafe { 
                let ti = pa.0 as *mut TaskInfo;
                *ti = TaskInfo {
                    status: TaskStatus::Running,
                    syscall_times: *current_syscall_info(),
                    time: (get_time_us() -  current_start_time()) / 1_000,
                }
            };
            0
        }
        None => -1,
    }
}

pub fn sys_set_priority(_prio: isize) -> isize {
    -1
}