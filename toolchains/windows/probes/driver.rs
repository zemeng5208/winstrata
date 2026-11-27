#![no_std]

use core::ffi::c_void;

unsafe extern "system" {
    fn KeQueryTimeIncrement() -> u32;
}

#[unsafe(no_mangle)]
pub extern "system" fn DriverEntry(
    _driver_object: *mut c_void,
    _registry_path: *mut c_void,
) -> i32 {
    // 只读取时钟间隔以验证真实的内核导入，不解引用入口参数或注册资源。
    let _increment = unsafe { KeQueryTimeIncrement() };
    // STATUS_NOT_SUPPORTED 使这个链接探针不会成功驻留在内核中。
    0xC00000BB_u32 as i32
}
