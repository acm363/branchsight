use std::io::{ErrorKind, Read};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

pub(crate) const DIFF_LIMIT: usize = 4 * 1024 * 1024;
pub(crate) const NETWORK_TIMEOUT: Duration = Duration::from_secs(90);
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
    pub limit: Option<usize>,
    pub accepted_exit: i32,
    pub timeout: Option<Duration>,
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

impl<'a> Call<'a> {
    pub fn new(args: &'a [&'a str]) -> Call<'a> {
        Call { args, limit: None, accepted_exit: 0, timeout: Some(READ_TIMEOUT) }
    }
}

pub(crate) fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    run(dir, &Call::new(args)).map(|output| output.text)
}

pub(crate) fn run(dir: &Path, call: &Call) -> Result<Output, String> {
    let mut child = command(dir, call.args).spawn().map_err(|e| {
        if e.kind() == ErrorKind::NotFound { "git is not installed or not on the PATH".to_owned() } else { e.to_string() }
    })?;
    let (sender, chunks) = mpsc::channel();
    forward(child.stdout.take().expect("stdout is piped"), Stream::Out, sender.clone());
    forward(child.stderr.take().expect("stderr is piped"), Stream::Err, sender);

    let started = Instant::now();
    let (mut stdout, mut stderr, mut truncated, mut exited) = (Vec::new(), Vec::new(), false, None);
    loop {
        match chunks.recv_timeout(TICK) {
            Ok((Stream::Out, bytes)) if !truncated => {
                stdout.extend_from_slice(&bytes);
                if let Some(limit) = call.limit.filter(|limit| stdout.len() > *limit) {
                    stdout.truncate(limit);
                    truncated = true;
                    stop(&mut child);
                }
            }
            Ok((Stream::Out, _)) => {}
            Ok((Stream::Err, bytes)) => stderr.extend_from_slice(&bytes[..bytes.len().min(STDERR_KEPT.saturating_sub(stderr.len()))]),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        if exited.is_none() {
            exited = child.try_wait().map_err(|e| e.to_string())?.map(|status| (status, Instant::now()));
        }
        if exited.is_some_and(|(_, at)| at.elapsed() > PIPES_GRACE) {
            break;
        }
        if let Some(limit) = call.timeout.filter(|limit| exited.is_none() && started.elapsed() > *limit) {
            stop(&mut child);
            return Err(format!("git {} took longer than {} s and was stopped", call.args[0], limit.as_secs()));
        }
    }
    let status = match exited {
        Some((status, _)) => status,
        None => child.wait().map_err(|e| e.to_string())?,
    };
    if truncated {
        return Ok(Output { text: decode(cut_at_line(stdout)), truncated });
    }
    let stderr = String::from_utf8_lossy(&stderr).trim().to_owned();
    if status.success() || status.code() == Some(call.accepted_exit) {
        Ok(Output { text: decode(stdout), truncated })
    } else if stderr.is_empty() {
        Err(format!("git {} failed ({status})", call.args[0]))
    } else {
        Err(stderr)
    }
}

fn command(dir: &Path, args: &[&str]) -> Command {
    let mut command = Command::new("git");
    command.arg("-C").arg(dir);
    for setting in PINNED_CONFIG {
        command.args(["-c", setting]);
    }
    command
        .args(args)
        .env_remove("LC_ALL")
        .env("LC_MESSAGES", "C")
        .env("GIT_TERMINAL_PROMPT", "0")
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
        thread::sleep(TICK);
    }
    unsafe { libc::kill(group, libc::SIGKILL) };
    let _ = child.wait();
}

#[cfg(windows)]
fn detach(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(windows)]
fn stop(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}
