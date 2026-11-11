//! Windows ConPTY backend: `CreatePseudoConsole` + `CreateProcessW` with the pseudo-console attribute.

use crate::conpty_bindings::{
    _PROC_THREAD_ATTRIBUTE_LIST, COORD, CREATE_UNICODE_ENVIRONMENT, CloseHandle, ClosePseudoConsole, CreatePipe,
    CreateProcessW, CreatePseudoConsole, DeleteProcThreadAttributeList, EXTENDED_STARTUPINFO_PRESENT,
    GetExitCodeProcess, HANDLE, HPCON, INFINITE, InitializeProcThreadAttributeList,
    PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE, PROCESS_INFORMATION, ResizePseudoConsole, STARTUPINFOEXW, TerminateProcess,
    UpdateProcThreadAttribute, WaitForSingleObject,
};
use crate::process_launch::{build_command_line, encode_environment_block, merge_environment};
use crate::pty::{
    EnvironmentVariable, ExitStatus, PtyBackend, PtyConnection, PtyControl, PtyError, PtySpawnRequest, TerminalSize,
};
use core::ffi::c_void;
use std::fs::File;
use std::os::windows::io::FromRawHandle;
use std::sync::{Arc, Mutex};
use std::thread;
use windows_core::{PCWSTR, PWSTR};

/// `STARTF_USESTDHANDLES`. Combined with invalid handles it stops the child inheriting our (possibly redirected) standard handles,
/// so it talks to the pseudo console instead.
const STARTF_USESTDHANDLES: u32 = 0x0000_0100;

pub struct ConPtyBackend;

impl PtyBackend for ConPtyBackend {
    fn spawn(&self, request: &PtySpawnRequest) -> Result<PtyConnection, PtyError> {
        tracing::debug!(columns = request.size.columns, rows = request.size.rows, "creating pseudo console");
        spawn_in_pseudo_console(request)
            .map_err(|reason| PtyError::SpawnFailed { program: request.program.clone(), reason })
    }
}

/// Closes the wrapped handle on drop unless released with `into_file`.
struct PipeEnd(HANDLE);

impl PipeEnd {
    fn into_file(self) -> File {
        let handle = self.0;
        std::mem::forget(self);
        unsafe { File::from_raw_handle(handle.0) }
    }
}

impl Drop for PipeEnd {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

struct ConPtyControl {
    process: HANDLE,
    console: Mutex<Option<HPCON>>,
}

// SAFETY: Win32 handles are process-wide identifiers and every operation used here is thread-safe.
unsafe impl Send for ConPtyControl {}
unsafe impl Sync for ConPtyControl {}

impl ConPtyControl {
    fn close_console(&self) {
        let console = self.console.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take();
        if let Some(console) = console {
            unsafe { ClosePseudoConsole(console) };
        }
    }
}

impl PtyControl for ConPtyControl {
    fn resize(&self, size: TerminalSize) -> Result<(), PtyError> {
        let console = self.console.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(console) = *console else { return Ok(()) };
        unsafe { ResizePseudoConsole(console, coordinate(size)) }.ok().map_err(|error| PtyError::OperationFailed {
            operation: "resize pseudo console",
            reason: error.to_string(),
        })
    }

    fn wait_for_exit(&self) -> ExitStatus {
        unsafe {
            WaitForSingleObject(self.process, INFINITE);
            let mut code = 0u32;
            let known = GetExitCodeProcess(self.process, &mut code).as_bool();
            ExitStatus { code: known.then_some(code as i32) }
        }
    }

    fn kill(&self) -> Result<(), PtyError> {
        unsafe { TerminateProcess(self.process, 1) }
            .ok()
            .map_err(|error| PtyError::OperationFailed { operation: "terminate process", reason: error.to_string() })
    }
}

impl Drop for ConPtyControl {
    fn drop(&mut self) {
        self.close_console();
        unsafe {
            let _ = CloseHandle(self.process);
        }
    }
}

fn coordinate(size: TerminalSize) -> COORD {
    COORD { X: size.columns.max(1) as i16, Y: size.rows.max(1) as i16 }
}

fn describe(operation: &str, error: windows_core::Error) -> String {
    format!("{operation}: {error}")
}

fn create_pipe() -> Result<(PipeEnd, PipeEnd), String> {
    let mut read = HANDLE::default();
    let mut write = HANDLE::default();
    unsafe { CreatePipe(&mut read, &mut write, None, 0) }.ok().map_err(|error| describe("CreatePipe", error))?;
    Ok((PipeEnd(read), PipeEnd(write)))
}

fn current_environment() -> Vec<EnvironmentVariable> {
    std::env::vars().map(|(name, value)| EnvironmentVariable { name, value }).collect()
}

fn spawn_in_pseudo_console(request: &PtySpawnRequest) -> Result<PtyConnection, String> {
    let (input_read, input_write) = create_pipe()?;
    let (output_read, output_write) = create_pipe()?;

    let console = unsafe { CreatePseudoConsole(coordinate(request.size), input_read.0, output_write.0, 0) }
        .map_err(|error| describe("CreatePseudoConsole", error))?;
    // The pseudo console duplicated its ends; ours must be closed or the output pipe never reports EOF.
    drop(input_read);
    drop(output_write);

    let process = match start_process(console, request) {
        Ok(process) => process,
        Err(reason) => {
            unsafe { ClosePseudoConsole(console) };
            return Err(reason);
        }
    };

    let control = Arc::new(ConPtyControl { process, console: Mutex::new(Some(console)) });

    // The output pipe only reports EOF after the pseudo console is closed, so close it when the shell exits.
    let watcher_control = Arc::clone(&control);
    thread::Builder::new()
        .name("conpty-exit-watcher".to_owned())
        .spawn(move || {
            watcher_control.wait_for_exit();
            watcher_control.close_console();
        })
        .map_err(|error| format!("spawn exit watcher: {error}"))?;

    Ok(PtyConnection { reader: Box::new(output_read.into_file()), writer: Box::new(input_write.into_file()), control })
}

fn start_process(console: HPCON, request: &PtySpawnRequest) -> Result<HANDLE, String> {
    let mut command_line: Vec<u16> =
        build_command_line(&request.program, &request.arguments).encode_utf16().chain(std::iter::once(0)).collect();
    let working_directory: Option<Vec<u16>> = request
        .working_directory
        .as_ref()
        .map(|directory| directory.to_string_lossy().encode_utf16().chain(std::iter::once(0)).collect());
    let environment_block = (!request.environment.is_empty())
        .then(|| encode_environment_block(&merge_environment(&current_environment(), &request.environment)));

    unsafe {
        let mut attribute_list_bytes = 0usize;
        // Expected to fail with ERROR_INSUFFICIENT_BUFFER; it reports the required size.
        let _ = InitializeProcThreadAttributeList(None, 1, None, &mut attribute_list_bytes);
        let mut attribute_storage = vec![0usize; attribute_list_bytes.div_ceil(size_of::<usize>())];
        let attribute_list = attribute_storage.as_mut_ptr().cast::<_PROC_THREAD_ATTRIBUTE_LIST>();
        InitializeProcThreadAttributeList(Some(attribute_list), 1, None, &mut attribute_list_bytes)
            .ok()
            .map_err(|error| describe("InitializeProcThreadAttributeList", error))?;

        let outcome = (|| {
            UpdateProcThreadAttribute(
                attribute_list,
                0,
                PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize,
                Some(console.0.cast_const()),
                size_of::<HPCON>(),
                None,
                None,
            )
            .ok()
            .map_err(|error| describe("UpdateProcThreadAttribute", error))?;

            let mut startup_info = STARTUPINFOEXW::default();
            startup_info.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
            let no_handle = HANDLE(-1isize as *mut c_void);
            startup_info.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
            startup_info.StartupInfo.hStdInput = no_handle;
            startup_info.StartupInfo.hStdOutput = no_handle;
            startup_info.StartupInfo.hStdError = no_handle;
            startup_info.lpAttributeList = attribute_list;
            let mut process_information = PROCESS_INFORMATION::default();

            CreateProcessW(
                PCWSTR::null(),
                Some(PWSTR(command_line.as_mut_ptr())),
                None,
                None,
                false,
                (EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT) as u32,
                environment_block.as_ref().map(|block| block.as_ptr().cast::<c_void>()),
                working_directory.as_ref().map_or(PCWSTR::null(), |directory| PCWSTR(directory.as_ptr())),
                (&raw const startup_info.StartupInfo).cast(),
                &mut process_information,
            )
            .ok()
            .map_err(|error| describe("CreateProcessW", error))?;

            let _ = CloseHandle(process_information.hThread);
            Ok(process_information.hProcess)
        })();

        DeleteProcThreadAttributeList(attribute_list);
        outcome
    }
}
