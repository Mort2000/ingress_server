#![forbid(unsafe_code)]

unsafe extern "C" {
    fn shm_open(name: *const u8, oflag: i32, mode: u32) -> i32;
}

pub fn open_region(name: &[u8]) -> i32 {
    unsafe { shm_open(name.as_ptr(), 0, 0) }
}
