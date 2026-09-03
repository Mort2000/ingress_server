//! sb-spin: burns CPU until killed (RLIMIT_CPU SIGXCPU or the
//! wall-clock watchdog).

#![forbid(unsafe_code)]

fn main() {
    loop {
        std::hint::spin_loop();
    }
}
