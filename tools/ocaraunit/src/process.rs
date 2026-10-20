// ─────────────────────────────────────────────────────────────────────────────
// Lancement d'un processus avec délai maximal
// ─────────────────────────────────────────────────────────────────────────────

use std::{
    io::Read,
    os::unix::process::ExitStatusExt,
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

/// Sortie d'un processus terminé (ou tué au bout du délai).
pub struct Output {
    pub status:    ExitStatus,
    pub stdout:    String,
    pub stderr:    String,
    pub timed_out: bool,
}

fn drain<R: Read + Send + 'static>(pipe: Option<R>) -> thread::JoinHandle<String> {
    thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut p) = pipe {
            let _ = p.read_to_end(&mut buf);
        }
        String::from_utf8_lossy(&buf).to_string()
    })
}

/// Attend `child` au plus `limit` ; au-delà, le tue.
fn wait_limited(child: &mut Child, limit: Duration) -> std::io::Result<(ExitStatus, bool)> {
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok((status, false));
        }
        if start.elapsed() >= limit {
            let _ = child.kill();
            return Ok((child.wait()?, true));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

/// Exécute `cmd` (stdout/stderr capturés) en le tuant après `limit`.
pub fn run(cmd: &mut Command, limit: Duration) -> std::io::Result<Output> {
    let mut child = cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    let out = drain(child.stdout.take());
    let err = drain(child.stderr.take());
    let (status, timed_out) = wait_limited(&mut child, limit)?;
    Ok(Output {
        status,
        stdout: out.join().unwrap_or_default(),
        stderr: err.join().unwrap_or_default(),
        timed_out,
    })
}

fn signal_name(sig: i32) -> &'static str {
    match sig {
        4 => "SIGILL", 6 => "SIGABRT", 7 => "SIGBUS", 8 => "SIGFPE",
        9 => "SIGKILL", 11 => "SIGSEGV", 13 => "SIGPIPE", 15 => "SIGTERM",
        _ => "",
    }
}

/// Fin anormale lisible : « signal 11 (SIGSEGV) » ou « code de sortie 3 ».
pub fn describe_status(status: &ExitStatus) -> String {
    match (status.signal(), status.code()) {
        (Some(sig), _) => format!("tué par le signal {} ({})", sig, signal_name(sig)).replace(" ()", ""),
        (None, Some(code)) => format!("code de sortie {}", code),
        _ => "fin anormale".to_string(),
    }
}

/// Retire les séquences d'échappement ANSI (`ESC [ … lettre`).
pub fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            for n in chars.by_ref() {
                if n.is_ascii_alphabetic() { break; }
            }
        } else {
            out.push(c);
        }
    }
    out
}
