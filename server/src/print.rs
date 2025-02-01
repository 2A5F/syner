use std::fmt::{Debug, Write};

#[cfg(target_os = "windows")]
pub fn sprint(str: impl AsRef<str>) -> anyhow::Result<()> {
    use windows::Win32::System::Console::*;
    use windows_strings::*;

    // let  data = encoding_rs::UTF_16LE.encode(str.as_ref());
    let h = HSTRING::from(str.as_ref());
    unsafe {
        let stdout = GetStdHandle(STD_OUTPUT_HANDLE)?;
        WriteConsoleW(stdout, &h, None, None)?;
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn sprint(str: impl AsRef<str>) -> anyhow::Result<()> {
    std::io::stdout().write(str.as_ref().as_bytes())?;
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn sprintf(fmt: std::fmt::Arguments<'_>) -> anyhow::Result<()> {
    let mut str = String::new();
    str.write_fmt(fmt)?;
    sprint(str)
}

#[cfg(not(target_os = "windows"))]
pub fn sprintf(fmt: std::fmt::Arguments<'_>) -> anyhow::Result<()> {
    use std::io::Write;

    std::io::stdout().write_fmt(fmt)?;
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn sprintfln(fmt: std::fmt::Arguments<'_>) -> anyhow::Result<()> {
    let mut str = String::new();
    str.write_fmt(fmt)?;
    str.write_str("\r\n")?;
    sprint(str)
}

#[cfg(not(target_os = "windows"))]
pub fn sprintfln(fmt: std::fmt::Arguments<'_>) -> anyhow::Result<()> {
    use std::io::Write;

    std::io::stdout().write_fmt(fmt)?;
    std::io::stdout().write("\n".as_bytes())?;
    Ok(())
}

#[macro_export]
macro_rules! sprint {
    ($($arg:tt)*) => {
        $crate::print::sprintf(::std::format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! sprintln {
    () => {
        $crate::sprint!("\n")
    };
    ($($arg:tt)*) => {
        $crate::print::sprintfln(::std::format_args!($($arg)*))
    };
}

#[cfg(target_os = "windows")]
pub fn s_read_line(str: &mut String) -> anyhow::Result<usize> {
    use windows::Win32::System::Console::*;
    use windows_strings::*;

    unsafe {
        let stdin = GetStdHandle(STD_INPUT_HANDLE)?;
        let mut buff = Vec::with_capacity(1024);

        let mut count = 0u32;

        let ctrl = CONSOLE_READCONSOLE_CONTROL {
            nLength: std::mem::size_of::<CONSOLE_READCONSOLE_CONTROL>() as u32,
            nInitialChars: 0,
            dwCtrlWakeupMask: '\n' as u32,
            dwControlKeyState: 0,
        };

        loop {
            ReadConsoleW(
                stdin,
                (buff.as_ptr() as usize + buff.len()) as _,
                1024,
                &mut count as _,
                Some(&ctrl as _),
            )?;
            buff.set_len(buff.len() + count as usize);
            if count != 1024 {
                break;
            }
            count = 0;
            buff.reserve(buff.len() + 1024);
        }

        if *buff.last().unwrap() == '\n' as u16 {
            buff.set_len(buff.len() - 1);
        }
        if *buff.last().unwrap() == '\r' as u16 {
            buff.set_len(buff.len() - 1);
        }
        *str = HSTRING::from_wide(&buff).to_string();
        Ok(str.len())
    }
}

#[cfg(not(target_os = "windows"))]
pub fn s_read_line(str: &mut String) -> anyhow::Result<usize> {
    let r = std::io::stdin().read_line(str)?;
    Ok(r)
}
pub fn read_line() -> anyhow::Result<String> {
    let mut str = String::new();
    let len = s_read_line(&mut str)?;
    let mut vec = str.into_bytes();
    unsafe {
        vec.set_len(len);
        Ok(String::from_utf8_unchecked(vec))
    }
}
