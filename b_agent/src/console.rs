#[cfg(target_os = "windows")]
pub unsafe fn free_console() {
    use winapi::um::wincon::FreeConsole;

    unsafe { FreeConsole() };
}

#[cfg(target_os = "windows")]
pub unsafe fn alloc_console() -> Result<(), std::io::Error> {
    use std::fs::OpenOptions;
    use std::os::windows::io::AsRawHandle;
    use winapi::um::consoleapi::AllocConsole;
    use winapi::um::processenv::SetStdHandle;
    use winapi::um::winbase::{STD_ERROR_HANDLE, STD_OUTPUT_HANDLE};

    // Try to allocate console
    let result = unsafe { AllocConsole() };
    if result == 0 {
        // Console allocation failed, but continue anyway
        eprintln!("AllocConsole failed, but continuing...");
    }

    // Try to redirect stdout
    match OpenOptions::new().write(true).read(true).open("CONOUT$") {
        Ok(stdout_file) => {
            let stdout_handle = stdout_file.as_raw_handle() as *mut winapi::ctypes::c_void;
            let result = unsafe { SetStdHandle(STD_OUTPUT_HANDLE, stdout_handle) };
            if result != 0 {
                // Keep file handle alive by leaking it
                std::mem::forget(stdout_file);
            }
        }
        Err(e) => {
            eprintln!("Failed to open CONOUT$ for stdout: {e:?}");
        }
    }

    // Try to redirect stderr
    match OpenOptions::new().write(true).read(true).open("CONOUT$") {
        Ok(stderr_file) => {
            let stderr_handle = stderr_file.as_raw_handle() as *mut winapi::ctypes::c_void;
            let result = unsafe { SetStdHandle(STD_ERROR_HANDLE, stderr_handle) };
            if result != 0 {
                // Keep file handle alive by leaking it
                std::mem::forget(stderr_file);
            }
        }
        Err(e) => {
            eprintln!("Failed to open CONOUT$ for stderr: {e:?}");
        }
    }

    println!("Console allocation completed!");

    Ok(())
}
