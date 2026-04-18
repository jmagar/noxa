use std::io;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::thread;
use std::time::Duration;

use tempfile::TempDir;

pub struct McpProcessHarness {
    _home: TempDir,
    child: Child,
}

impl McpProcessHarness {
    pub fn spawn() -> io::Result<Self> {
        let home = tempfile::tempdir()?;
        let child = Command::new(env!("CARGO_BIN_EXE_noxa-mcp"))
            .env("HOME", home.path())
            .env_remove("NOXA_API_KEY")
            .env_remove("SEARXNG_URL")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        Ok(Self { _home: home, child })
    }

    pub fn assert_running(&mut self) -> io::Result<()> {
        thread::sleep(Duration::from_millis(250));
        match self.child.try_wait()? {
            Some(status) => Err(io::Error::other(format!(
                "noxa-mcp exited early with status {status}"
            ))),
            None => Ok(()),
        }
    }

    pub fn stdin_mut(&mut self) -> Option<&mut ChildStdin> {
        self.child.stdin.as_mut()
    }

    pub fn stdout_mut(&mut self) -> Option<&mut ChildStdout> {
        self.child.stdout.as_mut()
    }
}

impl Drop for McpProcessHarness {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
