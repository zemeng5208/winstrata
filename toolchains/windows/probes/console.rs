#![no_std]

unsafe extern "system" {
    fn ExitProcess(exit_code: u32) -> !;
}

#[unsafe(no_mangle)]
pub extern "system" fn WinStrataEntry() -> ! {
    // ExitProcess 是 kernel32 的进程退出 API，此探针只返回退出码。
    unsafe { ExitProcess(0) }
}
