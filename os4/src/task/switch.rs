
core::arch::global_asm!(include_str!("switch.S"));

use super::TaskContext;

extern "C" {
    /// Switch to context of `next_task_cx_ptr`,saving the current context.
    ///in `current_task_cx_ptr`.
    pub fn __switch(current_task_cx_ptr: *mut TaskContext, next_task_cx_ptr: *const TaskContext);
}