//! PTY-backed terminal process management.

pub mod screen;

use std::{
    io::{self, Read, Write},
    path::Path,
    sync::mpsc::{self, Receiver},
    thread::{self, JoinHandle},
};

use portable_pty::{native_pty_system, Child, CommandBuilder, ExitStatus, MasterPty, PtySize};

pub use screen::{Cell, CellStyle, Color, ScreenBuffer};

/// A running shell attached to a pseudo terminal.
pub struct Terminal {
    master: Box<dyn MasterPty + Send>,
    writer: Option<Box<dyn Write + Send>>,
    child: Option<Box<dyn Child + Send + Sync>>,
    output_rx: Receiver<Vec<u8>>,
    reader_thread: Option<JoinHandle<()>>,
}

impl Terminal {
    /// Spawn a shell process attached to a PTY.
    pub fn spawn(cwd: impl AsRef<Path>, shell: Option<&str>) -> io::Result<Self> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(to_io_error)?;

        let shell_program = shell
            .map(ToOwned::to_owned)
            .unwrap_or_else(default_shell_program);

        let mut cmd = CommandBuilder::new(shell_program);
        cmd.cwd(cwd.as_ref());

        let child = pair.slave.spawn_command(cmd).map_err(to_io_error)?;
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader().map_err(to_io_error)?;
        let writer = pair.master.take_writer().map_err(to_io_error)?;
        let (output_tx, output_rx) = mpsc::channel();
        let reader_thread = thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if output_tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(err) => {
                        if err.kind() != io::ErrorKind::Interrupted {
                            break;
                        }
                    }
                }
            }
        });

        Ok(Self {
            master: pair.master,
            writer: Some(writer),
            child: Some(child),
            output_rx,
            reader_thread: Some(reader_thread),
        })
    }

    /// Read available output without blocking.
    pub fn read(&self) -> Vec<u8> {
        let mut out = Vec::new();
        while let Ok(chunk) = self.output_rx.try_recv() {
            out.extend_from_slice(&chunk);
        }
        out
    }

    /// Send input bytes to the PTY.
    pub fn write(&mut self, data: &[u8]) -> io::Result<()> {
        let Some(writer) = &mut self.writer else {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "terminal writer is unavailable",
            ));
        };
        writer.write_all(data)?;
        writer.flush()?;
        Ok(())
    }

    /// Resize the PTY dimensions.
    pub fn resize(&self, cols: u16, rows: u16) -> io::Result<()> {
        self.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(to_io_error)
    }

    /// Poll whether the shell process has exited.
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        match &mut self.child {
            Some(child) => child.try_wait(),
            None => Ok(Some(ExitStatus::with_exit_code(0))),
        }
    }

    /// Kill the shell process and clean up resources.
    pub fn kill(&mut self) -> io::Result<()> {
        self.writer.take();

        if let Some(child) = &mut self.child {
            child.kill()?;
            let _ = child.wait();
        }
        self.child.take();

        if let Some(handle) = self.reader_thread.take() {
            let _ = handle.join();
        }

        Ok(())
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = self.kill();
    }
}

fn to_io_error(err: impl std::fmt::Display) -> io::Error {
    io::Error::other(err.to_string())
}

fn default_shell_program() -> String {
    #[cfg(unix)]
    {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
    }
    #[cfg(windows)]
    {
        std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{thread, time::Duration};

    #[test]
    fn spawn_write_read_kill_round_trip() {
        let cwd = std::env::current_dir().expect("cwd");
        let mut terminal = Terminal::spawn(cwd, None).expect("spawn");
        terminal.write(b"echo TERMINAL_OK\r").expect("write");

        let mut output = String::new();
        for _ in 0..40 {
            let chunk = terminal.read();
            if !chunk.is_empty() {
                output.push_str(&String::from_utf8_lossy(&chunk));
                if output.contains("TERMINAL_OK") {
                    break;
                }
            }
            thread::sleep(Duration::from_millis(50));
        }

        assert!(
            output.contains("TERMINAL_OK"),
            "terminal output should include command result"
        );

        terminal.resize(100, 30).expect("resize");
        terminal.kill().expect("kill");
    }
}
