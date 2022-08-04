extern "C" {
    fn _exit(code: i32) -> !;
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum ExitCode {
    Success = 0,
    Failure = -1,
}

pub fn exit(code: ExitCode) -> ! {
    unsafe { _exit(code as i32) }
}
