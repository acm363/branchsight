use std::io::{ErrorKind, Read};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

pub(crate) const DIFF_LIMIT: usize = 4 * 1024 * 1024;
pub(crate) const ACTION_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const READ_TIMEOUT: Duration = Duration::from_secs(120);
const STDERR_KEPT: usize = 64 * 1024;
const TICK: Duration = Duration::from_millis(50);
const PIPES_GRACE: Duration = Duration::from_secs(2);
const STOP_GRACE: Duration = Duration::from_secs(3);

const PINNED_CONFIG: [&str; 13] = [
    "core.quotepath=off",
    "color.ui=never",
    "color.diff=never",
    "diff.noprefix=false",
    "diff.mnemonicPrefix=false",
    "diff.srcPrefix=a/",
    "diff.dstPrefix=b/",
    "diff.relative=false",
    "diff.submodule=short",
    "diff.suppressBlankEmpty=false",
    "log.showRoot=true",
    "log.abbrevCommit=false",
    "log.showSignature=false",
];

pub(crate) struct Call<'a> {
    pub args: &'a [&'a str],
    pub config: &'a [&'a str],
    pub limit: Option<usize>,
    pub accepted_exit: i32,
    pub timeout: Duration,
}

pub(crate) struct Output {
    pub text: String,
    pub truncated: bool,
}

#[derive(Clone, Copy)]
enum Stream {
    Out,
    Err,
}

#[derive(Default)]
struct Collected {
    stdout: Vec<u8>,
    stderr: Clipped,
    truncated: bool,
}

#[derive(Default)]
struct Clipped {
    head: Vec<u8>,
    tail: Vec<u8>,
    cut: bool,
}

impl<'a> Call<'a> {
    pub fn new(args: &'a [&'a str]) -> Call<'a> {
        Call { args, config: &[], limit: None, accepted_exit: 0, timeout: READ_TIMEOUT }
    }
}

impl Collected {
    fn take(&mut self, (stream, bytes): (Stream, Vec<u8>), limit: Option<usize>) {
        match stream {
            Stream::Out if !self.truncated => {
                self.stdout.extend_from_slice(&bytes);
                if let Some(limit) = limit.filter(|limit| self.stdout.len() > *limit) {
                    self.stdout.truncate(limit);
                    self.truncated = true;
                }
            }
            Stream::Out => {}
            Stream::Err => self.stderr.push(&bytes),
        }
    }
}

impl Clipped {
    const HALF: usize = STDERR_KEPT / 2;

    fn push(&mut self, bytes: &[u8]) {
        let room = Self::HALF.saturating_sub(self.head.len()).min(bytes.len());
        self.head.extend_from_slice(&bytes[..room]);
        self.tail.extend_from_slice(&bytes[room..]);
        if self.tail.len() > Self::HALF {
            self.tail.drain(..self.tail.len() - Self::HALF);
            self.cut = true;
        }
    }

    fn text(&self) -> String {
        if !self.cut {
            return String::from_utf8_lossy(&[&self.head[..], &self.tail[..]].concat()).trim().to_owned();
        }
        let head = &self.head[..self.head.iter().rposition(|b| *b == b'\n').unwrap_or(self.head.len())];
        let tail = &self.tail[self.tail.iter().position(|b| *b == b'\n').map_or(0, |i| i + 1)..];
        format!("{}\n…\n{}", String::from_utf8_lossy(head).trim_end(), String::from_utf8_lossy(tail).trim())
    }
}

pub(crate) fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    run(dir, &Call::new(args)).map(|output| output.text)
}

pub(crate) fn run(dir: &Path, call: &Call) -> Result<Output, String> {
    let mut child = command(dir, call).spawn().map_err(|e| {
        if e.kind() == ErrorKind::NotFound { "git is not installed or not on the PATH".to_owned() } else { e.to_string() }
    })?;
    let (sender, chunks) = mpsc::channel();
    forward(child.stdout.take().expect("stdout is piped"), Stream::Out, sender.clone());
    forward(child.stderr.take().expect("stderr is piped"), Stream::Err, sender);
    let (status, collected) = wait(&mut child, &chunks, call)?;
    if collected.truncated {
        return Ok(Output { text: decode(cut_at_line(collected.stdout)), truncated: true });
    }
    let stderr = collected.stderr.text();
    if status.success() || status.code() == Some(call.accepted_exit) {
        Ok(Output { text: decode(collected.stdout), truncated: false })
    } else if stderr.is_empty() {
        Err(format!("git {} failed ({status})", call.args[0]))
    } else {
        Err(stderr)
    }
}

fn wait(child: &mut Child, chunks: &Receiver<(Stream, Vec<u8>)>, call: &Call) -> Result<(ExitStatus, Collected), String> {
    let mut collected = Collected::default();
    let mut deadline = Instant::now() + call.timeout;
    let (mut status, mut pipes_open) = (None, true);
    loop {
        if pipes_open {
            match chunks.recv_timeout(TICK) {
                Ok(chunk) => collected.take(chunk, call.limit),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => pipes_open = false,
            }
        } else {
            thread::sleep(TICK);
        }
        if status.is_none() {
            status = child.try_wait().map_err(|e| e.to_string())?;
            if status.is_none() && collected.truncated {
                stop(child);
                status = child.try_wait().map_err(|e| e.to_string())?;
            }
            if status.is_some() {
                deadline = Instant::now() + PIPES_GRACE;
            }
        }
        match status {
            Some(exited) if !pipes_open || Instant::now() > deadline => {
                while let Ok(chunk) = chunks.try_recv() {
                    collected.take(chunk, call.limit);
                }
                return Ok((exited, collected));
            }
            None if Instant::now() > deadline => {
                stop(child);
                return Err(format!("git {} took longer than {} s and was stopped", call.args[0], call.timeout.as_secs()));
            }
            _ => {}
        }
    }
}

fn command(dir: &Path, call: &Call) -> Command {
    let mut command = Command::new("git");
    command.arg("-C").arg(dir);
    for setting in PINNED_CONFIG.iter().chain(call.config) {
        command.args(["-c", setting]);
    }
    command
        .args(call.args)
        .env_remove("LC_ALL")
        .env("LC_MESSAGES", "C")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_EDITOR", "true")
        .env("GIT_SEQUENCE_EDITOR", "true")
        .env("GIT_ASKPASS", "")
        .env("SSH_ASKPASS_REQUIRE", "never")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    detach(&mut command);
    command
}

fn forward(mut pipe: impl Read + Send + 'static, stream: Stream, sender: Sender<(Stream, Vec<u8>)>) {
    thread::spawn(move || {
        let mut chunk = [0u8; 64 * 1024];
        loop {
            match pipe.read(&mut chunk) {
                Ok(0) => return,
                Ok(n) if sender.send((stream, chunk[..n].to_vec())).is_ok() => {}
                Err(e) if e.kind() == ErrorKind::Interrupted => {}
                _ => return,
            }
        }
    });
}

fn cut_at_line(mut data: Vec<u8>) -> Vec<u8> {
    let end = data.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1);
    data.truncate(end);
    data
}

fn decode(data: Vec<u8>) -> String {
    String::from_utf8(data).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
}

#[cfg(unix)]
fn detach(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    unsafe {
        command.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
}

#[cfg(unix)]
fn stop(child: &mut Child) {
    let group = -(child.id() as libc::pid_t);
    unsafe { libc::kill(group, libc::SIGTERM) };
    let deadline = Instant::now() + STOP_GRACE;
    while Instant::now() < deadline {
        if let Ok(Some(_)) = child.try_wait() {
            return;
        }
        thread::sleep(Duration::from_millis(5));
    }
    unsafe { libc::kill(group, libc::SIGKILL) };
    let _ = child.wait();
}

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[cfg(windows)]
fn detach(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(windows)]
fn stop(child: &mut Child) {
    use std::os::windows::process::CommandExt;
    let tree = Command::new("taskkill").args(["/T", "/F", "/PID", &child.id().to_string()]).creation_flags(CREATE_NO_WINDOW).output();
    if !tree.is_ok_and(|out| out.status.success()) {
        let _ = child.kill();
    }
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_error_keeps_whole_lines_from_its_start_and_end() {
        // Given.
        let mut clipped = Clipped::default();
        let lines: String = (0..20_000).map(|n| format!("line {n}\n")).collect();

        // When.
        clipped.push(b"fatal: the reason\n");
        clipped.push(lines.as_bytes());
        let text = clipped.text();

        // Then.
        assert!(text.starts_with("fatal: the reason\n"));
        assert!(text.ends_with("line 19999"));
        assert!(text.lines().all(|line| line == "fatal: the reason" || line == "…" || line.starts_with("line ")), "no line is cut in two");
    }

    #[test]
    fn a_short_error_is_kept_as_it_is() {
        // Given.
        let mut clipped = Clipped::default();

        // When.
        clipped.push(b"error: one\n");
        clipped.push(b"error: two\n");

        // Then.
        assert_eq!(clipped.text(), "error: one\nerror: two");
    }
}
