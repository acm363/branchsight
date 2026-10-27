use std::io::{ErrorKind, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

pub(crate) const DIFF_LIMIT: usize = 4 * 1024 * 1024;
pub(crate) const NETWORK_TIMEOUT: Duration = Duration::from_secs(90);
const LOCAL_TIMEOUT: Duration = Duration::from_secs(120);
const STDERR_KEPT: usize = 64 * 1024;
const POLL: Duration = Duration::from_millis(2);

const PINNED_CONFIG: [&str; 7] = [
    "core.quotepath=off",
    "color.ui=never",
    "color.diff=never",
    "diff.noprefix=false",
    "diff.mnemonicPrefix=false",
    "diff.relative=false",
    "log.showSignature=false",
];

pub(crate) struct Call<'a> {
    pub args: &'a [&'a str],
    pub config: Option<&'a str>,
    pub limit: Option<usize>,
    pub accepted_exit: i32,
    pub timeout: Duration,
}

pub(crate) struct Output {
    pub text: String,
    pub truncated: bool,
}

impl<'a> Call<'a> {
    pub fn new(args: &'a [&'a str]) -> Call<'a> {
        Call { args, config: None, limit: None, accepted_exit: 0, timeout: LOCAL_TIMEOUT }
    }
}

pub(crate) fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    run(dir, &Call::new(args)).map(|output| output.text)
}

pub(crate) fn run(dir: &Path, call: &Call) -> Result<Output, String> {
    let mut command = base_command();
    command.arg("-C").arg(dir);
    for setting in PINNED_CONFIG.iter().copied().chain(call.config) {
        command.args(["-c", setting]);
    }
    let mut child = command
        .args(call.args)
        .env("LC_ALL", "C")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| if e.kind() == ErrorKind::NotFound { "git is not installed or not on the PATH".into() } else { e.to_string() })?;

    let overflow = Arc::new(AtomicBool::new(false));
    let stdout = child.stdout.take().expect("stdout is piped");
    let stderr = child.stderr.take().expect("stderr is piped");
    let reader = {
        let (overflow, limit) = (Arc::clone(&overflow), call.limit);
        thread::spawn(move || read_capped(stdout, limit, &overflow))
    };
    let drainer = thread::spawn(move || drain(stderr));

    let deadline = Instant::now() + call.timeout;
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break Some(status);
        }
        if overflow.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("git {} took longer than {} s and was stopped", call.args[0], call.timeout.as_secs()));
        }
        thread::sleep(POLL);
    };

    let stdout = reader.join().unwrap_or_default();
    let stderr = String::from_utf8_lossy(&drainer.join().unwrap_or_default()).trim().to_owned();
    if overflow.load(Ordering::Relaxed) {
        return Ok(Output { text: decode(cut_at_line(stdout)), truncated: true });
    }
    match status {
        Some(status) if status.success() || status.code() == Some(call.accepted_exit) => Ok(Output { text: decode(stdout), truncated: false }),
        Some(status) if stderr.is_empty() => Err(format!("git {} failed ({status})", call.args[0])),
        _ => Err(stderr),
    }
}

fn read_capped(mut pipe: impl Read, limit: Option<usize>, overflow: &AtomicBool) -> Vec<u8> {
    let mut data = Vec::new();
    let mut chunk = [0u8; 64 * 1024];
    loop {
        match pipe.read(&mut chunk) {
            Ok(0) => return data,
            Ok(n) => {
                data.extend_from_slice(&chunk[..n]);
                if let Some(limit) = limit.filter(|limit| data.len() > *limit) {
                    data.truncate(limit);
                    overflow.store(true, Ordering::Relaxed);
                    return data;
                }
            }
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(_) => return data,
        }
    }
}

fn drain(mut pipe: impl Read) -> Vec<u8> {
    let mut kept = Vec::new();
    let mut chunk = [0u8; 16 * 1024];
    loop {
        match pipe.read(&mut chunk) {
            Ok(0) => return kept,
            Ok(n) => kept.extend_from_slice(&chunk[..n.min(STDERR_KEPT.saturating_sub(kept.len()))]),
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(_) => return kept,
        }
    }
}

fn cut_at_line(mut data: Vec<u8>) -> Vec<u8> {
    let end = data.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1);
    data.truncate(end);
    data
}

fn decode(data: Vec<u8>) -> String {
    String::from_utf8(data).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
}

#[cfg(windows)]
fn base_command() -> Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut command = Command::new("git");
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

#[cfg(not(windows))]
fn base_command() -> Command {
    Command::new("git")
}
