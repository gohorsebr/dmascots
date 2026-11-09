//! Talk to the running instance over a per-user local socket (Unix socket / Windows named pipe).
//!
//! Same endpoint and protocol as the Python app's QLocalServer, so either side's CLI can
//! drive either app. One JSON object per line in each direction:
//!
//! ```text
//! {"cmd": "say", "text": "...", "to": "all" | "<mascot>", "seconds": float | null}
//! {"cmd": "spawn", "mascots": ["monkey", ...], "lifetime": float | null, "say": str | null}
//! {"cmd": "steal", "to": "any" | "<mascot>", "seconds": float}
//! {"cmd": "quit"}
//! ```
//!
//! Replies: `{"ok": true, "count": N}` or `{"ok": false, "error": "..."}`

use std::io::{BufRead, BufReader, Write};
use std::sync::mpsc;
use std::time::Duration;

use serde_json::{json, Value};

/// A request from a client, with the channel its reply goes back on.
pub struct Request {
    pub body: Value,
    pub reply: mpsc::Sender<Value>,
}

/// `DESKTOP_MASCOTS_SOCKET` picks a different control socket, i.e. a separate, independent
/// instance (handy for testing without touching the mascots you already have running).
pub fn server_name() -> String {
    if let Ok(name) = std::env::var("DESKTOP_MASCOTS_SOCKET") {
        if !name.is_empty() {
            return name;
        }
    }
    let user: String = user_name()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || "_.-".contains(c) { c } else { '_' })
        .collect();
    format!("desktop-mascots-{user}")
}

/// Python's getpass.getuser(): the first of LOGNAME, USER, LNAME, USERNAME, else the account.
fn user_name() -> String {
    for var in ["LOGNAME", "USER", "LNAME", "USERNAME"] {
        if let Ok(v) = std::env::var(var) {
            if !v.is_empty() {
                return v;
            }
        }
    }
    #[cfg(unix)]
    {
        // SAFETY: getpwuid returns a pointer into static storage (or null)
        unsafe {
            let pw = libc::getpwuid(libc::getuid());
            if !pw.is_null() && !(*pw).pw_name.is_null() {
                return std::ffi::CStr::from_ptr((*pw).pw_name).to_string_lossy().into_owned();
            }
        }
    }
    "user".into()
}

/// Where QLocalServer puts a socket with this name: $TMPDIR (or /tmp) on Unix, a named pipe on Windows.
fn endpoint(name: &str) -> String {
    #[cfg(unix)]
    {
        if name.starts_with('/') {
            return name.to_string();
        }
        let tmp = std::env::var("TMPDIR").ok().filter(|t| !t.is_empty()).unwrap_or_else(|| "/tmp".into());
        format!("{}/{name}", tmp.trim_end_matches('/'))
    }
    #[cfg(windows)]
    {
        format!(r"\\.\pipe\{name}")
    }
}

fn exchange<S: std::io::Read + Write>(stream: S, message: &Value) -> Value {
    let mut stream = BufReader::new(stream);
    let mut line = message.to_string();
    line.push('\n');
    if stream.get_mut().write_all(line.as_bytes()).and_then(|_| stream.get_mut().flush()).is_err() {
        return json!({"ok": false, "error": "couldn't send to the running instance"});
    }
    let mut reply = String::new();
    match stream.read_line(&mut reply) {
        Ok(n) if n > 0 => serde_json::from_str(&reply)
            .unwrap_or_else(|_| json!({"ok": false, "error": "bad reply from the running instance"})),
        _ => json!({"ok": false, "error": "no reply from the running instance"}),
    }
}

/// Send one request to the running instance. None if nothing is listening.
pub fn send(message: &Value) -> Option<Value> {
    let path = endpoint(&server_name());
    let timeout = Duration::from_millis(1500);
    #[cfg(unix)]
    {
        let stream = std::os::unix::net::UnixStream::connect(&path).ok()?;
        let _ = stream.set_read_timeout(Some(timeout));
        let _ = stream.set_write_timeout(Some(timeout));
        Some(exchange(stream, message))
    }
    #[cfg(windows)]
    {
        let start = std::time::Instant::now();
        loop {
            match std::fs::OpenOptions::new().read(true).write(true).open(&path) {
                Ok(f) => return Some(exchange(f, message)),
                // all pipe instances busy: the server is between clients, retry briefly
                Err(e) if e.raw_os_error() == Some(231) && start.elapsed() < timeout => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(_) => return None,
            }
        }
    }
}

/// Serve requests on a background thread; each one is handed to the main loop through
/// the returned channel. Err if the endpoint can't be opened.
pub fn serve() -> Result<mpsc::Receiver<Request>, String> {
    let (tx, rx) = mpsc::channel();
    let path = endpoint(&server_name());
    #[cfg(unix)]
    {
        use std::os::unix::net::UnixListener;
        let _ = std::fs::remove_file(&path); // clear a stale socket left by a crash
        // only this user may connect: create the socket with mode 0600
        // SAFETY: umask only changes the process file-creation mask
        let old = unsafe { libc::umask(0o177) };
        let listener = UnixListener::bind(&path);
        unsafe { libc::umask(old) };
        let listener = listener.map_err(|e| format!("{path}: {e}"))?;
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let tx = tx.clone();
                std::thread::spawn(move || {
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
                    if let Ok(w) = stream.try_clone() {
                        handle_client(BufReader::new(stream), w, &tx);
                    }
                });
            }
        });
    }
    #[cfg(windows)]
    {
        let first = win_pipe::create(&path, true).map_err(|e| format!("{path}: {e}"))?;
        std::thread::spawn(move || {
            let mut next = Some(first);
            loop {
                let pipe = match next.take() {
                    Some(p) => p,
                    None => match win_pipe::create(&path, false) {
                        Ok(p) => p,
                        Err(_) => return,
                    },
                };
                if !win_pipe::accept(&pipe) {
                    continue;
                }
                let tx = tx.clone();
                std::thread::spawn(move || {
                    if let Ok(w) = pipe.try_clone() {
                        handle_client(BufReader::new(pipe), w, &tx);
                    }
                });
            }
        });
    }
    Ok(rx)
}

/// Remove the socket file on a clean exit (Unix; pipes vanish with the process).
pub fn cleanup() {
    #[cfg(unix)]
    {
        let _ = std::fs::remove_file(endpoint(&server_name()));
    }
}

fn handle_client<R: BufRead, W: Write>(mut reader: R, mut writer: W, tx: &mpsc::Sender<Request>) {
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        let reply = match serde_json::from_str::<Value>(line.trim()) {
            Ok(body) if body.is_object() => {
                let (rtx, rrx) = mpsc::channel();
                if tx.send(Request { body, reply: rtx }).is_err() {
                    return; // app is shutting down
                }
                rrx.recv_timeout(Duration::from_secs(5))
                    .unwrap_or_else(|_| json!({"ok": false, "error": "the app didn't answer"}))
            }
            _ => json!({"ok": false, "error": "bad request"}),
        };
        let mut out = reply.to_string();
        out.push('\n');
        if writer.write_all(out.as_bytes()).and_then(|_| writer.flush()).is_err() {
            return;
        }
    }
}

#[cfg(windows)]
mod win_pipe {
    //! Named pipe server, with a DACL that lets only the owner (this user) connect.
    use std::fs::File;
    use std::os::windows::io::FromRawHandle;

    use windows_sys::Win32::Foundation::{GetLastError, ERROR_PIPE_CONNECTED, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Security::Authorization::{
        ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
    };
    use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
    use windows_sys::Win32::Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX};
    use windows_sys::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE,
        PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn create(path: &str, first: bool) -> Result<File, String> {
        // OW = OWNER RIGHTS (whoever created the pipe), SY = LocalSystem
        let sddl = wide("D:P(A;;GA;;;OW)(A;;GA;;;SY)");
        let mut sd = std::ptr::null_mut();
        // SAFETY: plain Win32 calls with valid, NUL-terminated buffers
        unsafe {
            if ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &mut sd,
                std::ptr::null_mut(),
            ) == 0
            {
                return Err(format!("security descriptor error {}", GetLastError()));
            }
            let sa = SECURITY_ATTRIBUTES {
                nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: sd,
                bInheritHandle: 0,
            };
            let flags = PIPE_ACCESS_DUPLEX | if first { FILE_FLAG_FIRST_PIPE_INSTANCE } else { 0 };
            let name = wide(path);
            let h = CreateNamedPipeW(
                name.as_ptr(),
                flags,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                PIPE_UNLIMITED_INSTANCES,
                4096,
                4096,
                0,
                &sa,
            );
            windows_sys::Win32::Foundation::LocalFree(sd as _);
            if h == INVALID_HANDLE_VALUE {
                return Err(format!("CreateNamedPipe error {}", GetLastError()));
            }
            Ok(File::from_raw_handle(h as _))
        }
    }

    /// Wait for a client on this pipe instance.
    pub fn accept(pipe: &File) -> bool {
        use std::os::windows::io::AsRawHandle;
        // SAFETY: the handle is a valid pipe handle owned by `pipe`
        unsafe {
            ConnectNamedPipe(pipe.as_raw_handle() as _, std::ptr::null_mut()) != 0
                || GetLastError() == ERROR_PIPE_CONNECTED
        }
    }
}
