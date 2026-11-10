use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Mutex;

use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use tauri::ipc::Channel;
use tauri::State;

#[derive(serde::Serialize)]
pub struct ShellInfo {
    id: String,
    label: String,
}

struct Session {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
}

#[derive(Default)]
pub struct Terminal(Mutex<HashMap<u32, Session>>);

impl Terminal {
    fn remove(&self, id: u32) {
        if let Some(mut session) = self.0.lock().unwrap().remove(&id) {
            let _ = session.child.kill();
            let _ = session.child.wait();
        }
    }
}

#[tauri::command]
pub fn term_shells() -> Vec<ShellInfo> {
    shells()
}

#[tauri::command]
pub fn term_open(
    term: State<'_, Terminal>,
    id: u32,
    shell: Option<String>,
    cols: u16,
    rows: u16,
    cwd: Option<String>,
    on_data: Channel<String>,
) -> Result<(), String> {
    term.remove(id);
    let size = PtySize { rows: rows.max(1), cols: cols.max(1), pixel_width: 0, pixel_height: 0 };
    let pair = native_pty_system().openpty(size).map_err(|e| e.to_string())?;

    let shell = shell.filter(|s| !s.is_empty() && shells().iter().any(|sh| sh.id == *s)).unwrap_or_else(default_shell);
    let mut cmd = CommandBuilder::new(shell);
    if let Some(dir) = cwd.filter(|d| !d.is_empty()) {
        cmd.cwd(dir);
    }
    cmd.env("TERM", "xterm-256color");
    let child = pair.slave.spawn_command(cmd).map_err(|e| e.to_string())?;
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
    let writer = pair.master.take_writer().map_err(|e| e.to_string())?;

    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if on_data.send(base64(&buf[..n])).is_err() {
                        break;
                    }
                }
            }
        }
    });

    term.0.lock().unwrap().insert(id, Session { master: pair.master, writer, child });
    Ok(())
}

#[tauri::command]
pub fn term_write(term: State<'_, Terminal>, id: u32, data: String) -> Result<(), String> {
    let mut guard = term.0.lock().unwrap();
    if let Some(session) = guard.get_mut(&id) {
        session.writer.write_all(data.as_bytes()).map_err(|e| e.to_string())?;
        let _ = session.writer.flush();
    }
    Ok(())
}

#[tauri::command]
pub fn term_resize(term: State<'_, Terminal>, id: u32, cols: u16, rows: u16) -> Result<(), String> {
    let guard = term.0.lock().unwrap();
    if let Some(session) = guard.get(&id) {
        let size = PtySize { rows: rows.max(1), cols: cols.max(1), pixel_width: 0, pixel_height: 0 };
        session.master.resize(size).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn term_close(term: State<'_, Terminal>, id: u32) {
    term.remove(id);
}

#[tauri::command]
pub fn term_external(cwd: Option<String>) -> Result<(), String> {
    launch_external(cwd.filter(|d| !d.is_empty()).as_deref())
}

fn which(name: &str) -> Option<String> {
    if name.contains(std::path::MAIN_SEPARATOR) || name.contains('/') {
        return Path::new(name).is_file().then(|| name.to_owned());
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|full| full.is_file())
        .map(|full| full.to_string_lossy().into_owned())
}

#[cfg(unix)]
fn default_shell() -> String {
    std::env::var("SHELL").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "/bin/bash".to_owned())
}

#[cfg(windows)]
fn default_shell() -> String {
    which("pwsh.exe")
        .or_else(|| which("powershell.exe"))
        .unwrap_or_else(|| "cmd.exe".to_owned())
}

fn shell_info(id: String) -> ShellInfo {
    let label = Path::new(&id)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| id.clone());
    ShellInfo { id, label }
}

#[cfg(unix)]
fn shells() -> Vec<ShellInfo> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    let candidates = std::iter::once(default_shell())
        .chain(["bash", "zsh", "fish", "sh"].iter().filter_map(|n| which(n)));
    for id in candidates {
        if seen.insert(id.clone()) {
            out.push(shell_info(id));
        }
    }
    out
}

#[cfg(windows)]
fn shells() -> Vec<ShellInfo> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    let git_bash = std::env::var("ProgramFiles")
        .ok()
        .map(|p| format!("{p}\\Git\\bin\\bash.exe"))
        .filter(|p| Path::new(p).is_file());
    let candidates = [default_shell()]
        .into_iter()
        .chain(["pwsh.exe", "powershell.exe", "cmd.exe"].iter().filter_map(|n| which(n)))
        .chain(git_bash);
    for id in candidates {
        if seen.insert(id.to_lowercase()) {
            out.push(shell_info(id));
        }
    }
    out
}

#[cfg(target_os = "linux")]
fn launch_external(dir: Option<&str>) -> Result<(), String> {
    use std::process::Command;
    let dir = dir.filter(|d| Path::new(d).is_dir()).unwrap_or(".");
    let mut candidates: Vec<(String, Vec<String>)> = Vec::new();
    if let Ok(term) = std::env::var("TERMINAL") {
        if !term.is_empty() {
            candidates.push((term, vec![]));
        }
    }
    candidates.push(("x-terminal-emulator".to_owned(), vec![]));
    candidates.push(("gnome-terminal".to_owned(), vec![format!("--working-directory={dir}")]));
    candidates.push(("konsole".to_owned(), vec!["--workdir".to_owned(), dir.to_owned()]));
    candidates.push(("xfce4-terminal".to_owned(), vec![format!("--working-directory={dir}")]));
    candidates.push(("kitty".to_owned(), vec!["--directory".to_owned(), dir.to_owned()]));
    candidates.push(("alacritty".to_owned(), vec!["--working-directory".to_owned(), dir.to_owned()]));
    candidates.push(("xterm".to_owned(), vec![]));

    for (program, args) in &candidates {
        if which(program).is_none() {
            continue;
        }
        if Command::new(program).args(args).current_dir(dir).spawn().is_ok() {
            return Ok(());
        }
    }
    Err("no terminal emulator found".to_owned())
}

#[cfg(target_os = "macos")]
fn launch_external(dir: Option<&str>) -> Result<(), String> {
    let dir = dir.filter(|d| Path::new(d).is_dir()).unwrap_or(".");
    std::process::Command::new("open")
        .args(["-a", "Terminal", dir])
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(target_os = "windows")]
fn launch_external(dir: Option<&str>) -> Result<(), String> {
    use std::process::Command;
    let dir = dir.filter(|d| Path::new(d).is_dir());
    let mut wt = Command::new("wt");
    if let Some(d) = dir {
        wt.args(["-d", d]);
    }
    if wt.spawn().is_ok() {
        return Ok(());
    }
    let mut cmd = Command::new("cmd");
    cmd.args(["/c", "start", "", "cmd"]);
    if let Some(d) = dir {
        cmd.current_dir(d);
    }
    cmd.spawn().map(|_| ()).map_err(|e| e.to_string())
}

fn base64(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[(n >> 18 & 63) as usize] as char);
        out.push(ALPHABET[(n >> 12 & 63) as usize] as char);
        out.push(if chunk.len() > 1 { ALPHABET[(n >> 6 & 63) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { ALPHABET[(n & 63) as usize] as char } else { '=' });
    }
    out
}
