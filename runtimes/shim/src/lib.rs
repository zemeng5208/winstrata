//! 解释器加载后、用户脚本运行前收紧进程。
//!
//! 宿主先准备 Landlock 规则，并把规则集文件描述符传给本库。构造函数在 Python 或 Node 的主函数之前执行。

#[cfg(target_os = "linux")]
mod linux {
    use core::ffi::{c_char, c_int};

    const SYS_WRITE: isize = 1;
    const SYS_CLOSE: isize = 3;
    const SYS_CLOSE_RANGE: isize = 436;
    const SYS_LANDLOCK_RESTRICT_SELF: isize = 446;
    const PR_SET_NO_NEW_PRIVS: isize = 38;
    const PR_SET_SECCOMP: isize = 22;
    const SECCOMP_MODE_FILTER: isize = 2;
    const LD_W_ABS: u16 = 0x20;
    const JMP_JEQ_K: u16 = 0x15;
    const RET_K: u16 = 0x06;
    const AUDIT_ARCH_X86_64: u32 = 0xc000_003e;
    const SECCOMP_RET_ALLOW: u32 = 0x7fff_0000;
    const SECCOMP_RET_KILL_PROCESS: u32 = 0x8000_0000;
    const SECCOMP_RET_ERRNO: u32 = 0x0005_0000;
    const EPERM: u32 = 1;

    // 允许线程所需的 clone。拒绝新程序、套接字和命名空间切换。
    // 不拒绝 prctl：过滤只能更严，脚本不能借它恢复被拒绝的调用。
    const DENY: [u32; 35] = [
        41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 57, 58, 59, 101, 165, 166, 246,
        272, 288, 298, 308, 310, 311, 320, 321, 322, 323, 425, 426, 427,
    ];

    #[derive(Clone, Copy)]
    #[repr(C)]
    struct Filter {
        code: u16,
        jt: u8,
        jf: u8,
        k: u32,
    }

    #[repr(C)]
    struct Program {
        len: u16,
        _pad: u16,
        filter: *const Filter,
    }

    unsafe extern "C" {
        fn getenv(name: *const c_char) -> *const c_char;
        fn _exit(code: c_int) -> !;
        fn syscall(number: isize, ...) -> isize;
        fn prctl(option: c_int, ...) -> c_int;
    }

    unsafe fn syscall3(number: isize, a: isize, b: isize, c: isize) -> isize {
        unsafe { syscall(number, a, b, c) }
    }

    fn fail(step: u8) -> ! {
        fail_num(step, 0);
    }

    fn fail_num(step: u8, code: isize) -> ! {
        let mut bytes = [b'!', step, b':', b'-', b'0', b'0', b'0', b'\n'];
        let mut value = if code < 0 { -code } else { code } as u32;
        for index in (4..7).rev() {
            bytes[index] = b'0' + (value % 10) as u8;
            value /= 10;
        }
        unsafe {
            syscall3(SYS_WRITE, 2, bytes.as_ptr() as isize, bytes.len() as isize);
            _exit(1);
        }
    }

    fn ruleset_fd() -> i32 {
        let name = c"WINSTRATA_LANDLOCK_FD".as_ptr();
        let value = unsafe { getenv(name) };
        if value.is_null() {
            fail(b'e');
        }
        let mut fd = 0i32;
        let mut saw_digit = false;
        unsafe {
            let mut cursor = value;
            while *cursor != 0 {
                let byte = *cursor as u8;
                if !byte.is_ascii_digit() {
                    fail(b'd');
                }
                saw_digit = true;
                fd = fd
                    .checked_mul(10)
                    .and_then(|value| value.checked_add((byte - b'0') as i32))
                    .unwrap_or_else(|| fail(b'd'));
                cursor = cursor.add(1);
            }
        }
        if !saw_digit {
            fail(b'd');
        }
        fd
    }

    fn restrict(fd: i32) {
        let restricted = unsafe { syscall3(SYS_LANDLOCK_RESTRICT_SELF, fd as isize, 0, 0) };
        if restricted != 0 {
            fail_num(b'r', restricted);
        }
        unsafe { syscall3(SYS_CLOSE, fd as isize, 0, 0) };
    }

    fn instruction(code: u16, jt: u8, jf: u8, k: u32) -> Filter {
        Filter { code, jt, jf, k }
    }

    fn engage_seccomp() {
        if cfg!(not(target_arch = "x86_64")) {
            fail(b'x');
        }
        let mut program = [instruction(0, 0, 0, 0); 48];
        program[0] = instruction(LD_W_ABS, 0, 0, 4);
        program[2] = instruction(LD_W_ABS, 0, 0, 0);
        let mut count = 3usize;
        for syscall in DENY {
            program[count] = instruction(JMP_JEQ_K, 0, 0, syscall);
            count += 1;
        }
        let allow = count;
        program[count] = instruction(RET_K, 0, 0, SECCOMP_RET_ALLOW);
        count += 1;
        let deny = count;
        program[count] = instruction(RET_K, 0, 0, SECCOMP_RET_ERRNO | EPERM);
        count += 1;
        let kill = count;
        program[count] = instruction(RET_K, 0, 0, SECCOMP_RET_KILL_PROCESS);
        count += 1;
        for index in 3..allow {
            let offset = deny - (index + 1);
            program[index].jt = u8::try_from(offset).unwrap_or_else(|_| fail(b'x'));
        }
        program[1] = instruction(
            JMP_JEQ_K,
            0,
            u8::try_from(kill - 2).unwrap_or_else(|_| fail(b'x')),
            AUDIT_ARCH_X86_64,
        );
        let header = Program {
            len: u16::try_from(count).unwrap_or_else(|_| fail(b'x')),
            _pad: 0,
            filter: program.as_ptr(),
        };
        let installed = unsafe {
            prctl(
                PR_SET_SECCOMP as c_int,
                SECCOMP_MODE_FILTER as u64,
                &header as *const Program as u64,
                0u64,
                0u64,
            )
        };
        if installed != 0 {
            fail_num(b's', installed as isize);
        }
    }

    fn close_inherited() {
        // 保留标准输入、输出和错误。其余描述符，包括宿主打开的文件，在脚本运行前关闭。
        unsafe { syscall3(SYS_CLOSE_RANGE, 3, isize::MAX, 0) };
    }

    pub fn engage() {
        let fd = ruleset_fd();
        // Landlock 要求先放下新特权，否则 restrict_self 返回 EPERM。
        let lowered = unsafe { prctl(PR_SET_NO_NEW_PRIVS as c_int, 1u64, 0u64, 0u64, 0u64) };
        if lowered != 0 {
            fail_num(b'n', lowered as isize);
        }
        restrict(fd);
        engage_seccomp();
        close_inherited();
    }
}

#[cfg(target_os = "linux")]
#[used]
#[unsafe(link_section = ".init_array.65535")]
static ENGAGE: unsafe extern "C" fn() = engage_entry;

#[cfg(target_os = "linux")]
unsafe extern "C" fn engage_entry() {
    linux::engage();
}

#[cfg(not(target_os = "linux"))]
#[unsafe(no_mangle)]
pub extern "C" fn winstrata_shim_present() {}
