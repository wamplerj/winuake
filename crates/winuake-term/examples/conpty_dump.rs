//! Diagnostic: prints the raw bytes a real ConPTY produces for `cmd.exe`. Windows only.

#[cfg(windows)]
fn main() {
    use std::io::{Read, Write};
    use std::time::Duration;
    use winuake_term::conpty::ConPtyBackend;
    use winuake_term::pty::{PtyBackend, PtySpawnRequest, TerminalSize};

    let request = PtySpawnRequest {
        program: "cmd.exe".to_owned(),
        arguments: vec![],
        working_directory: None,
        environment: vec![],
        size: TerminalSize::new(80, 25),
    };
    let connection = ConPtyBackend.spawn(&request).expect("spawn");
    let mut reader = connection.reader;
    let mut writer = connection.writer;
    std::thread::spawn(move || {
        let mut buffer = [0u8; 4096];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => {
                    println!("[eof]");
                    break;
                }
                Ok(length) => println!("[{length} bytes] {:?}", String::from_utf8_lossy(&buffer[..length])),
                Err(error) => {
                    println!("[error {error}]");
                    break;
                }
            }
        }
    });
    std::thread::sleep(Duration::from_secs(3));
    println!("-- writing command");
    writer.write_all(b"echo hi\r").unwrap();
    std::thread::sleep(Duration::from_secs(3));
    println!("-- done");
}

#[cfg(not(windows))]
fn main() {}
