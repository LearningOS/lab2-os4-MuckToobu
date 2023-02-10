
mod context;

use riscv::register::{utvec::TrapMode, stvec, sie, stval,
    scause::{self, Trap, Exception, Interrupt}};
pub use context::TrapContext;
use crate::{task::{current_trap_cx, current_user_token, exit_current_and_run_next, suspend_current_and_run_next,
    current_syscall_plus}, syscall::syscall, timer::set_next_trigger};


use crate::config::{TRAMPOLINE, TRAP_CONTEXT};

core::arch::global_asm!(include_str!("trap.S"));

pub fn init() {
    set_kernel_trap_entry();
}
pub fn enable_timer_interrupt() {
    unsafe {
        sie::set_stimer();
    }
}
#[no_mangle]
pub fn trap_from_kernel() -> ! {
    panic!("a trap from kernel!");
}
fn set_kernel_trap_entry() {
    unsafe {
        stvec::write(trap_from_kernel as usize, TrapMode::Direct);
    }
}
fn set_user_trap_entry() {
    unsafe {
        stvec::write(TRAMPOLINE as usize, TrapMode::Direct);
    }
}
#[no_mangle]
pub fn trap_handler() -> ! {
    set_kernel_trap_entry();
    let cx = current_trap_cx();
    let scause = scause::read();
    let stval = stval::read();
    match scause.cause() {
        Trap::Exception(Exception::UserEnvCall) => {
            cx.sepc += 4;
            current_syscall_plus(cx.x[17]);
            cx.x[10] = syscall(cx.x[17], [cx.x[10], cx.x[11], cx.x[12]]) as usize;
        },
        Trap::Exception(
            Exception::StoreFault | 
            Exception::StorePageFault |
            Exception::LoadPageFault
        ) => {
            error!("[kernel] IllegalInstruction in application, core dumped,");
            exit_current_and_run_next();
        }
        Trap::Exception(Exception::IllegalInstruction) => {
            error!("[kernel] IllegalInstruction in application, core dumped.");
            exit_current_and_run_next();
        }
        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            set_next_trigger();
            suspend_current_and_run_next();
        }
        _ => panic!(
            "Unsupported trap {:?}, stval = {:#x}!",
            scause.cause(),
            stval
        ),
    }
    trap_return();
}
#[no_mangle]
pub fn trap_return() -> ! {
    set_user_trap_entry();
    let trap_cx_ptr = TRAP_CONTEXT;
    let user_satp = current_user_token();
    extern "C" {
        fn __alltraps();
        fn __restore();
    }
    let restore_va = __restore as usize - __alltraps as usize + TRAMPOLINE;
    unsafe {
        core::arch::asm!(
            "fence.i",
            "jr {restore_va}",
            restore_va = in(reg) restore_va,
            in("a0") trap_cx_ptr,
            in("a1") user_satp,
            options(noreturn),
        );
    }
}