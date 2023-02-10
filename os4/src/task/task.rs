
use alloc::boxed::Box;

use super::TaskContext;
use crate::trap::{trap_handler, TrapContext};
use crate::config::{kernel_stack_position, TRAP_CONTEXT, MAX_SYSCALL_NUM};
use crate::mm::{MapPermission, MemorySet, PhysPageNum, VirtAddr, 
    KERNEL_SPACE};

    #[derive(Copy, Clone, PartialEq)]
pub enum TaskStatus {
    UnInit,
    Ready,
    Running,
    Exited,
}

/// this block controls current task.
/// `base_size` -> this task stack's top, that means
/// the base_size indicates the size of the app space
/// from 0x0 to user stack end.
pub struct TaskControlBlock {
    pub task_status: TaskStatus,
    pub task_cx: TaskContext,
    pub time: usize,
    pub syscall_times: Box<[u32; MAX_SYSCALL_NUM]>,
    pub memory_set: MemorySet,
    pub trap_cx_ppn: PhysPageNum,
    pub base_size: usize,
}

//------------------impl struct--------------------//
impl TaskControlBlock {
    pub fn get_trap_cx(&self) -> &'static mut TrapContext {
        self.trap_cx_ppn.get_mut()
    }
    pub fn get_user_token(&self) -> usize {
        self.memory_set.token()
    }
    pub fn new(elf_data: &[u8], app_id: usize) -> Self {
        // memory_set with elf program headers/trampoline/trap_context/user_stack
        let (memory_set, user_sp, entry_point) = MemorySet::from_elf(elf_data);
        let trap_cx_ppn = memory_set
            .translate(VirtAddr(TRAP_CONTEXT).into())
            .unwrap()
            .ppn();
        let task_status = TaskStatus::Ready;
        // map a kernel-stack in kernel space
        let (kernel_stack_botton, kernel_stack_top) = kernel_stack_position(app_id);
        KERNEL_SPACE.exclusive_access().insert_framed_area(
            VirtAddr(kernel_stack_botton),
            VirtAddr(kernel_stack_top),
            MapPermission::R | MapPermission::W,
        );
        let task_control_block = TaskControlBlock {
            task_status,
            task_cx: TaskContext::goto_trap_return(kernel_stack_top),
            time: 0,
            syscall_times: Box::new([0; MAX_SYSCALL_NUM]),
            memory_set,
            trap_cx_ppn,
            base_size: user_sp.0,
        };
        // preapare TrapContext in user space
        let trap_cx = task_control_block.get_trap_cx();
        *trap_cx = TrapContext::app_init_context(
            entry_point.0,
            user_sp.0,
            KERNEL_SPACE.exclusive_access().token(),
            kernel_stack_top,
            trap_handler as usize,
        );
        task_control_block
    }
}