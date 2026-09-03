#![forbid(unsafe_code)]

pub fn raw() -> u8 {
    let v = 7u8;
    unsafe { *std::ptr::addr_of!(v) }
}
