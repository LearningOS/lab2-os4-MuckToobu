


use alloc::boxed::Box;
use alloc::vec::Vec;
use lazy_static::lazy_static;

use super::{TaskControlBlock, TaskStatus, TaskContext,
    __switch};
use crate::config::MAX_SYSCALL_NUM;
use crate::mm::{VirtAddr, PhysAddr, MapPermission};
use crate::sync::UPSafeCell;
use crate::timer::get_time_us;
use crate::trap::TrapContext;
use crate::loader::{get_num_app, get_app_data};

pub struct TaskManagerInner {
    tasks: Vec<TaskControlBlock>,
    current_task: usize,
}

pub struct TaskManager {
    num_app: usize,
    inner: UPSafeCell<TaskManagerInner>,
}

lazy_static! {
    pub static ref TASK_MANAGER: TaskManager = {
        info!("init TASK_MANAGER");
        let num_app = get_num_app();
        info!("num_app = {}", num_app);
        let mut tasks: Vec<TaskControlBlock> = Vec::new();
        for i in 0..num_app {
            tasks.push(TaskControlBlock::new(get_app_data(i), i));
        }
        TaskManager {
            num_app,
            inner: unsafe {
                UPSafeCell::new(TaskManagerInner {
                    tasks,
                    current_task: 0,
                })
            },
        }
    };
}

impl TaskManager {
    fn run_first_task(&self) -> ! {
        let mut inner = self.inner.exclusive_access();
        let next_task = &mut inner.tasks[0];
        next_task.task_status = TaskStatus::Running;
        next_task.time = get_time_us();
        let next_task_cx_ptr = &next_task.task_cx as *const TaskContext;
        drop(inner);
        let mut _unused = TaskContext::zero_init();
        // before this, we should drop local variables that dropped manaully
        unsafe {
            __switch(&mut _unused as *mut _, next_task_cx_ptr);
        }
        panic!("unreachable in run_first_tasks!");
    }

    fn mark_current_suspended(&self) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].task_status = TaskStatus::Ready;
    }
    fn mark_current_exited(&self) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].task_status = TaskStatus::Exited;
    }
    fn find_next_task(&self) -> Option<usize> {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        (current + 1..=current + self.num_app)
            .map(|id| id % self.num_app)
            .find(|id| inner.tasks[*id].task_status == TaskStatus::Ready)
    }
    fn get_current_token(&self) -> usize {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].get_user_token()
    }
    fn get_current_trap_cx(&self) -> &mut TrapContext {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].get_trap_cx()
    }
    fn current_start_time(&self) -> usize {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].time
    }
    fn current_syscall_plus(&self, syscall: usize) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].syscall_times[syscall] += 1;
    }
    fn current_syscall_info(&self) -> Box<[u32; MAX_SYSCALL_NUM]>{
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].syscall_times.clone()
    }
    fn translate_addr_current_unchecked(&self, va: VirtAddr) -> Option<PhysAddr> {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].memory_set.translate_addr_unchecked(va)
    }
    fn current_map_crate(&self, start: VirtAddr, len: usize, port: MapPermission) -> isize {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].memory_set.map_create(start, len, port)
    }
    fn current_munmap(&self, start: VirtAddr, len: usize) -> isize {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].memory_set.munmap(start, len)
    }
    fn run_next_task(&self) {
        if let Some(next) = self.find_next_task() {
            let mut inner = self.inner.exclusive_access();
            let current = inner.current_task;
            inner.tasks[next].task_status = TaskStatus::Running;
            if inner.tasks[next].time == 0 {
                inner.tasks[next].time = get_time_us();
            } 
            inner.current_task = next;
            let current_task_cx_ptr = &mut inner.tasks[current].task_cx as *mut TaskContext;
            let next_task_cx_ptr = &inner.tasks[next].task_cx as *const TaskContext;
            drop(inner);
            
            unsafe {
                __switch(current_task_cx_ptr, next_task_cx_ptr);
            }
        } else {
            panic!("All application completed!");
        }
    }
}


fn run_next_task() {
    TASK_MANAGER.run_next_task();
}
fn mark_current_suspended() {
    TASK_MANAGER.mark_current_suspended();
}
fn mark_current_exited() {
    TASK_MANAGER.mark_current_exited();
}
pub fn run_first_task() {
    TASK_MANAGER.run_first_task();
}
pub fn suspend_current_and_run_next() {
    mark_current_suspended();
    run_next_task();
}
pub fn exit_current_and_run_next() {
    mark_current_exited();
    run_next_task();
}
pub fn current_user_token() -> usize {
    TASK_MANAGER.get_current_token()
}
pub fn current_trap_cx() -> &'static mut TrapContext {
    TASK_MANAGER.get_current_trap_cx()
}
pub fn translate(va: VirtAddr) -> Option<PhysAddr> {
    TASK_MANAGER.translate_addr_current_unchecked(va)
}
pub fn current_syscall_plus(syscall: usize) {
    TASK_MANAGER.current_syscall_plus(syscall);
}
pub fn current_syscall_info() -> Box<[u32; MAX_SYSCALL_NUM]> {
    TASK_MANAGER.current_syscall_info()
}
pub fn current_start_time() -> usize {
    TASK_MANAGER.current_start_time()
}
pub fn current_map_crate(start: VirtAddr, len: usize, port: MapPermission) -> isize {
    TASK_MANAGER.current_map_crate(start, len, port)
}
pub fn current_munmap(start: VirtAddr, len: usize) -> isize {
    TASK_MANAGER.current_munmap(start, len)
}