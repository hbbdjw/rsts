extern crate winapi;
use std::io::Error;
/// 打印Windows消息框
///
/// # Arguments
///
/// * `msg` - 要显示的消息字符串
///
/// # Returns
///
/// * `Ok(ret)` - 成功显示消息框，返回消息框的结果代码
/// * `Err(e)` - 显示消息框失败，返回错误信息
#[cfg(windows)]
pub fn print_message(msg: &str) -> Result<i32, Error> {
    use std::ffi::OsStr;
    use std::iter::once;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::null_mut;
    use winapi::um::winuser::{MB_OK, MessageBoxW};
    let wide: Vec<u16> = OsStr::new(msg).encode_wide().chain(once(0)).collect();
    let ret = unsafe { MessageBoxW(null_mut(), wide.as_ptr(), wide.as_ptr(), MB_OK) };
    if ret == 0 {
        Err(Error::last_os_error())
    } else {
        Ok(ret)
    }
}
#[cfg(not(windows))]
pub fn print_message(msg: &str) -> Result<(), Error> {
    println!("{}", msg);
    Ok(())
}
