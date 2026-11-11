//! The scripting pipe `\\.\pipe\winuake-<user name>`. Each connection is served on its own thread; every request line is handed to the UI
//! thread, because the workspace and the terminal views live there, and the reply is written back when the UI thread is done.
//!
//! Only the current user (and SYSTEM) may open the pipe, and the first instance is created with `FILE_FLAG_FIRST_PIPE_INSTANCE` so another
//! process cannot squat on the name.

use crate::bindings::{DispatcherQueue, DispatcherQueueHandler};
use crate::window_bindings::{
    CloseHandle, ConnectNamedPipe, ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
    CreateNamedPipeW, DisconnectNamedPipe, ERROR_PIPE_CONNECTED, FILE_FLAG_FIRST_PIPE_INSTANCE, FlushFileBuffers,
    GetCurrentProcess, GetTokenInformation, HANDLE, INVALID_HANDLE_VALUE, LocalFree, OpenProcessToken,
    PIPE_ACCESS_DUPLEX, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT, PSECURITY_DESCRIPTOR,
    PSID, ReadFile, SDDL_REVISION_1, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER, TokenUser, WriteFile,
};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use windows_core::{HSTRING, PWSTR};

const BUFFER_BYTES: u32 = 64 * 1024;
const MAXIMUM_REQUEST_BYTES: usize = 1024 * 1024;
const UI_REPLY_TIMEOUT: Duration = Duration::from_secs(15);

/// Name of this user's pipe.
pub fn pipe_path() -> String {
    winuake_ipc::names::pipe_path(&std::env::var("USERNAME").unwrap_or_default())
}

/// The user's SID as text, for the pipe's access list.
fn current_user_sid() -> Option<String> {
    unsafe {
        let mut token = HANDLE::default();
        if !OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY as u32, &mut token).as_bool() {
            return None;
        }
        let mut needed = 0u32;
        let _ = GetTokenInformation(token, TokenUser, None, 0, &mut needed);
        let mut buffer = vec![0u8; needed as usize];
        let read =
            GetTokenInformation(token, TokenUser, Some(buffer.as_mut_ptr().cast()), needed, &mut needed).as_bool();
        let _ = CloseHandle(token);
        if !read {
            return None;
        }
        let user = &*(buffer.as_ptr() as *const TOKEN_USER);
        let mut text = PWSTR::null();
        if !ConvertSidToStringSidW(PSID(user.User.Sid.0), &mut text).as_bool() {
            return None;
        }
        let sid = text.to_string().ok();
        let _ = LocalFree(HANDLE(text.0.cast()));
        sid
    }
}

/// Starts the server thread. `dispatcher` is the UI thread's queue.
pub fn start(dispatcher: DispatcherQueue) {
    let Some(sid) = current_user_sid() else {
        tracing::error!("scripting pipe not started: cannot find the user's SID");
        return;
    };
    let path = pipe_path();
    let spawned = thread::Builder::new().name("script-pipe".to_owned()).spawn(move || serve(&path, &sid, &dispatcher));
    if let Err(error) = spawned {
        tracing::error!(%error, "could not start the scripting pipe thread");
    }
}

fn serve(path: &str, sid: &str, dispatcher: &DispatcherQueue) {
    let name = HSTRING::from(path);
    // Protected DACL: full access for the user and SYSTEM only.
    let sddl = HSTRING::from(format!("D:P(A;;GA;;;{sid})(A;;GA;;;SY)"));
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    if !unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(&sddl, SDDL_REVISION_1 as u32, &mut descriptor, None)
    }
    .as_bool()
    {
        tracing::error!(error = %windows_core::Error::from_thread(), "scripting pipe not started: bad security descriptor");
        return;
    }
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: false.into(),
    };
    let mut first = true;
    loop {
        let mut open_mode = PIPE_ACCESS_DUPLEX as u32;
        if first {
            open_mode |= FILE_FLAG_FIRST_PIPE_INSTANCE as u32;
        }
        let pipe = unsafe {
            CreateNamedPipeW(
                &name,
                open_mode,
                (PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT) as u32,
                PIPE_UNLIMITED_INSTANCES as u32,
                BUFFER_BYTES,
                BUFFER_BYTES,
                0,
                Some(&attributes),
            )
        };
        if pipe == INVALID_HANDLE_VALUE {
            tracing::error!(error = %windows_core::Error::from_thread(), path, "could not create the scripting pipe");
            return;
        }
        if first {
            tracing::info!(path, "scripting pipe listening");
            first = false;
        }
        let connected = unsafe { ConnectNamedPipe(pipe, None) }.as_bool()
            || windows_core::Error::from_thread().code().0 as u32 & 0xFFFF == ERROR_PIPE_CONNECTED as u32;
        if !connected {
            let _ = unsafe { CloseHandle(pipe) };
            continue;
        }
        let handle = pipe.0 as isize;
        let queue = dispatcher.clone();
        if let Err(error) =
            thread::Builder::new().name("script-connection".to_owned()).spawn(move || handle_connection(handle, &queue))
        {
            tracing::error!(%error, "could not start a scripting connection thread");
            let _ = unsafe { CloseHandle(pipe) };
        }
    }
}

fn handle_connection(handle: isize, dispatcher: &DispatcherQueue) {
    let pipe = HANDLE(handle as *mut core::ffi::c_void);
    let mut pending: Vec<u8> = Vec::new();
    'connection: loop {
        let mut chunk = vec![0u8; 4096];
        let mut read = 0u32;
        let ok = unsafe { ReadFile(pipe, Some(chunk.as_mut_ptr().cast()), chunk.len() as u32, Some(&mut read), None) }
            .as_bool();
        if !ok || read == 0 {
            break;
        }
        pending.extend_from_slice(&chunk[..read as usize]);
        if pending.len() > MAXIMUM_REQUEST_BYTES {
            tracing::warn!("scripting request too large; closing the connection");
            break;
        }
        while let Some(newline) = pending.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = pending.drain(..=newline).collect();
            let text = String::from_utf8_lossy(&line[..line.len() - 1]).trim_end_matches('\r').to_owned();
            if text.is_empty() {
                continue;
            }
            if let Some(reply) = run_on_ui_thread(dispatcher, text) {
                let mut bytes = reply.into_bytes();
                bytes.push(b'\n');
                if !write_all(pipe, &bytes) {
                    break 'connection;
                }
            }
        }
    }
    unsafe {
        let _ = FlushFileBuffers(pipe);
        let _ = DisconnectNamedPipe(pipe);
        let _ = CloseHandle(pipe);
    }
}

fn write_all(pipe: HANDLE, bytes: &[u8]) -> bool {
    let mut offset = 0;
    while offset < bytes.len() {
        let mut written = 0u32;
        let ok = unsafe {
            WriteFile(
                pipe,
                Some(bytes[offset..].as_ptr().cast()),
                (bytes.len() - offset) as u32,
                Some(&mut written),
                None,
            )
        }
        .as_bool();
        if !ok || written == 0 {
            return false;
        }
        offset += written as usize;
    }
    true
}

/// Runs one request on the UI thread and waits for its reply. `None` for notifications.
fn run_on_ui_thread(dispatcher: &DispatcherQueue, line: String) -> Option<String> {
    let (sender, receiver) = mpsc::channel::<Option<String>>();
    let sender = std::sync::Mutex::new(Some(sender));
    let line = std::sync::Mutex::new(Some(line));
    let handler = DispatcherQueueHandler::new(move || {
        let line = line.lock().ok().and_then(|mut slot| slot.take());
        let sender = sender.lock().ok().and_then(|mut slot| slot.take());
        if let (Some(line), Some(sender)) = (line, sender) {
            let _ = sender.send(crate::workspace_host::run_script_line(&line));
        }
        Ok(())
    });
    if !dispatcher.TryEnqueue(&handler).unwrap_or(false) {
        tracing::warn!("could not hand a scripting request to the UI thread");
        return Some(winuake_ipc::protocol::error_line(
            &serde_json::Value::Null,
            &winuake_ipc::protocol::RpcError::new(winuake_ipc::protocol::REFUSED, "Winuake is busy or shutting down"),
        ));
    }
    match receiver.recv_timeout(UI_REPLY_TIMEOUT) {
        Ok(reply) => reply,
        Err(_) => Some(winuake_ipc::protocol::error_line(
            &serde_json::Value::Null,
            &winuake_ipc::protocol::RpcError::new(winuake_ipc::protocol::REFUSED, "Winuake did not answer in time"),
        )),
    }
}
