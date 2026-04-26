// https://degaart.github.io/20230123.html

use core::ffi::c_void;
use core::fmt;

unsafe extern "C" {
    fn write(fildes: i32, buf: *const c_void, nbyte: usize);
}

pub fn write_string(s: &str) {
    unsafe {
        write(1, s.as_ptr() as *const c_void, s.len());
    }
}

pub struct Writer;

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> Result<(), fmt::Error> {
        write_string(s);
        Ok(())
    }
}

pub fn print(args: fmt::Arguments) {
    use fmt::Write;
    Writer {}.write_fmt(args).unwrap();
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        $crate::print(format_args!($($arg)*));
    }}
}

#[macro_export]
macro_rules! println {
    () => {{
        print!("\n");
    }};

    ($($arg:tt)*) => {{
        print!("{}\n", format_args!($($arg)*));
    }}
}
