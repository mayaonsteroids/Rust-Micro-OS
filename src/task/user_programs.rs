//! Built-in Ring 3 User Programs and Isolated Execution Engine.
//!
//! Provides predefined user-space programs (`hello`, `counter`, `calc`) executing in
//! hardware-isolated address spaces (independent CR3, supervisor-only kernel protection,
//! user virtual memory mapping, user stack) and returning cleanly to the shell on `SYS_EXIT`.

use core::sync::atomic::Ordering;
use crate::interrupts::syscall::{
    run_user_code_with_exit_return, LAST_USER_EXIT_CODE,
};
use crate::memory::address_space::{
    restore_kernel_address_space, AddressSpace, USER_CODE_VIRT_ADDR, USER_STACK_TOP,
};

/// Specification of a built-in Ring 3 user program.
pub struct UserProgram {
    pub name: &'static str,
    pub description: &'static str,
    pub code: &'static [u8],
    pub data: &'static [u8],
}

/// Machine code for `hello`:
/// 1. `SYS_WRITE` (syscall 1): user data at `0x0000_0080_0002_0000`, length 39.
/// 2. `SYS_EXIT` (syscall 3): exit code 0.
pub static HELLO_CODE: [u8; 35] = [
    0xb8, 0x01, 0x00, 0x00, 0x00,                         // mov eax, 1 (SYS_WRITE)
    0x48, 0xbf, 0x00, 0x00, 0x02, 0x00, 0x80, 0x00, 0x00, 0x00, // movabs rdi, USER_DATA_VIRT_ADDR
    0x48, 0xc7, 0xc6, 0x27, 0x00, 0x00, 0x00,             // mov rsi, 39 (length)
    0xcd, 0x80,                                           // int 0x80
    0xb8, 0x03, 0x00, 0x00, 0x00,                         // mov eax, 3 (SYS_EXIT)
    0x31, 0xff,                                           // xor edi, edi (exit 0)
    0xcd, 0x80,                                           // int 0x80
    0xeb, 0xfe,                                           // jmp $
];

pub static HELLO_DATA: &[u8] = b"Hello from isolated Ring 3 user space!\n";

/// Machine code for `counter`:
/// 1. Initialize loop counter `ebx = 5`.
/// 2. Loop: call `SYS_YIELD` (syscall 2) via `int 0x80`, decrement `ebx`, jump if nonzero.
/// 3. Call `SYS_WRITE` (syscall 1) to print completion message (32 bytes).
/// 4. Call `SYS_EXIT` (syscall 3) with exit code 0.
pub static COUNTER_CODE: [u8; 51] = [
    0xbb, 0x05, 0x00, 0x00, 0x00,                         // mov ebx, 5
    // .loop:
    0xb8, 0x02, 0x00, 0x00, 0x00,                         // mov eax, 2 (SYS_YIELD)
    0xcd, 0x80,                                           // int 0x80
    0xff, 0xcb,                                           // dec ebx
    0x75, 0xf5,                                           // jne .loop (-11 bytes)
    // Print completion:
    0xb8, 0x01, 0x00, 0x00, 0x00,                         // mov eax, 1 (SYS_WRITE)
    0x48, 0xbf, 0x00, 0x00, 0x02, 0x00, 0x80, 0x00, 0x00, 0x00, // movabs rdi, USER_DATA_VIRT_ADDR
    0x48, 0xc7, 0xc6, 0x20, 0x00, 0x00, 0x00,             // mov rsi, 32 (length)
    0xcd, 0x80,                                           // int 0x80
    // Exit:
    0xb8, 0x03, 0x00, 0x00, 0x00,                         // mov eax, 3 (SYS_EXIT)
    0x31, 0xff,                                           // xor edi, edi (exit 0)
    0xcd, 0x80,                                           // int 0x80
    0xeb, 0xfe,                                           // jmp $
];

pub static COUNTER_DATA: &[u8] = b"Counter program finished (5/5).\n";

/// Machine code for `calc`:
/// 1. Multiply 42 * 10 = 420, add 7 = 427 in user registers (EAX).
/// 2. Call `SYS_WRITE` (syscall 1) to print formatted result message (45 bytes).
/// 3. Call `SYS_EXIT` (syscall 3) with exit code 0.
pub static CALC_CODE: [u8; 50] = [
    0xb8, 0x2a, 0x00, 0x00, 0x00,                         // mov eax, 42
    0xb9, 0x0a, 0x00, 0x00, 0x00,                         // mov ecx, 10
    0xf7, 0xe9,                                           // imul ecx (eax = 420)
    0x83, 0xc0, 0x07,                                     // add eax, 7 (eax = 427)
    // Print result:
    0xb8, 0x01, 0x00, 0x00, 0x00,                         // mov eax, 1 (SYS_WRITE)
    0x48, 0xbf, 0x00, 0x00, 0x02, 0x00, 0x80, 0x00, 0x00, 0x00, // movabs rdi, USER_DATA_VIRT_ADDR
    0x48, 0xc7, 0xc6, 0x2d, 0x00, 0x00, 0x00,             // mov rsi, 45 (length)
    0xcd, 0x80,                                           // int 0x80
    // Exit:
    0xb8, 0x03, 0x00, 0x00, 0x00,                         // mov eax, 3 (SYS_EXIT)
    0x31, 0xff,                                           // xor edi, edi (exit 0)
    0xcd, 0x80,                                           // int 0x80
    0xeb, 0xfe,                                           // jmp $
];

pub static CALC_DATA: &[u8] = b"Calc: 42 * 10 + 7 = 427 (verified in Ring 3)\n";

/// Catalog of all built-in user programs.
pub static USER_PROGRAMS: [UserProgram; 3] = [
    UserProgram {
        name: "hello",
        description: "Outputs greeting from Ring 3 via SYS_WRITE and exits cleanly",
        code: &HELLO_CODE,
        data: HELLO_DATA,
    },
    UserProgram {
        name: "counter",
        description: "Runs 5-iteration loop yielding CPU via SYS_YIELD in Ring 3",
        code: &COUNTER_CODE,
        data: COUNTER_DATA,
    },
    UserProgram {
        name: "calc",
        description: "Performs register arithmetic (42 * 10 + 7 = 427) in Ring 3",
        code: &CALC_CODE,
        data: CALC_DATA,
    },
];

/// Global tracking of the currently executing foreground user program name.
pub static ACTIVE_USER_PROGRAM: spin::Mutex<Option<&'static str>> = spin::Mutex::new(None);

/// RAII guard ensuring `ACTIVE_USER_PROGRAM` is unconditionally cleared upon scope exit.
struct ActiveProgramGuard;

impl Drop for ActiveProgramGuard {
    fn drop(&mut self) {
        *ACTIVE_USER_PROGRAM.lock() = None;
    }
}

/// Returns the complete catalog of registered user programs.
pub fn get_user_programs() -> &'static [UserProgram] {
    &USER_PROGRAMS
}

/// Finds a user program by its name.
pub fn find_user_program(name: &str) -> Option<&'static UserProgram> {
    USER_PROGRAMS.iter().find(|p| p.name == name)
}

/// Executes a user program inside a newly constructed, hardware-isolated address space.
///
/// 1. Constructs an independent Level 4 Page Table hierarchy (CR3).
/// 2. Loads program code into the user code page (`0x0000_0080_0000_0000`).
/// 3. Loads static data into the user data page (`0x0000_0080_0002_0000`).
/// 4. Switches CR3 to the process address space.
/// 5. Transitions CPU to Ring 3 via `iretq` and executes user instructions.
/// 6. Catches `SYS_EXIT` termination via vector 0x80 kernel trampoline.
/// 7. Restores kernel CR3.
/// 8. Destroys user address space, recycling all physical frames back to the global allocator.
/// 9. Returns the exit code recorded during termination.
pub fn run_user_program(name: &str) -> Result<u64, &'static str> {
    let prog = find_user_program(name).ok_or("Program not found")?;

    crate::info!("[RUN] Initializing isolated address space for '{}'...", prog.name);

    // 1. Construct isolated hardware address space
    let mut aspace = AddressSpace::new().map_err(|_| "Failed to construct address space")?;
    let cr3_frame = aspace.cr3;
    crate::info!("[RUN] Assigned isolated CR3 physical frame: 0x{:x}", cr3_frame.start_address().as_u64());

    // 2. Load executable machine code and data into user virtual pages
    aspace.load_code(prog.code).map_err(|_| "Failed to load code")?;
    aspace.load_data(prog.data).map_err(|_| "Failed to load data")?;

    crate::info!("[RUN] Transitioning to Ring 3 (User Mode) via iretq...");

    // 3. Switch active CR3 to user process page tables
    aspace.switch_to();

    // Reset any stale scheduler task indicator so scheduler cannot be confused
    crate::scheduler::round_robin::SCHEDULER.lock().current = None;

    *ACTIVE_USER_PROGRAM.lock() = Some(prog.name);
    let _active_guard = ActiveProgramGuard;

    // 4. Execute user code in Ring 3 with exit return trampoline
    unsafe {
        run_user_code_with_exit_return(USER_CODE_VIRT_ADDR, USER_STACK_TOP);
    }

    // Explicit drop of active guard clears ACTIVE_USER_PROGRAM
    drop(_active_guard);

    // 5. Restore primary kernel address space
    restore_kernel_address_space();

    // 6. Retrieve exit code
    let exit_code = LAST_USER_EXIT_CODE.load(Ordering::SeqCst);
    crate::info!("[RUN] Program '{}' completed with exit code {}.", prog.name, exit_code);

    // 7. Deallocate address space and recycle all owned physical frames
    aspace.destroy();
    crate::info!("[RUN] Destroyed address space and reclaimed all physical frames.");

    Ok(exit_code)
}
