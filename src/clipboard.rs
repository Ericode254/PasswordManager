use arboard::Clipboard;
use std::io::{BufRead, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use zeroize::Zeroizing;

struct OwnedHelper {
    child: Child,
    control: ChildStdin,
    session: ClipboardSession,
}
type Helper = Arc<Mutex<OwnedHelper>>;
static ACTIVE_HELPER: Mutex<Option<Helper>> = Mutex::new(None);

#[derive(Clone, Default)]
pub struct ClipboardSession(Arc<AtomicBool>);
impl ClipboardSession {
    pub fn revoked(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
    pub fn revoke(&self) {
        // Serialize revocation with registration of a newly started copy.
        let active = ACTIVE_HELPER.lock().unwrap_or_else(|e| e.into_inner());
        self.0.store(true, Ordering::SeqCst);
        if let Some(helper) = &*active
            && let Ok(mut helper) = helper.lock()
            && Arc::ptr_eq(&helper.session.0, &self.0)
        {
            let _ = helper.control.write_all(b"C");
        }
    }
}

pub struct ClipboardManager;
impl ClipboardManager {
    pub fn new() -> Self {
        Self
    }

    /// The helper owns expiration after normal exit; revocation requests early cleanup.
    pub fn copy_for_session(
        &self,
        text: &str,
        lifetime: Duration,
        session: &ClipboardSession,
    ) -> Result<(), String> {
        if session.revoked() {
            return Err("Session cleared; copy cancelled".into());
        }
        let executable = std::env::current_exe().map_err(|e| e.to_string())?;
        let mut command = Command::new(executable);
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        let expires = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "Invalid system clock")?
            + lifetime.saturating_sub(Duration::from_millis(20));
        let mut child = command
            .arg("--clipboard-helper")
            .arg(expires.as_millis().to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Cannot start clipboard helper: {e}"))?;
        let mut control = child.stdin.take().ok_or("Missing clipboard input")?;
        let result = (|| {
            control
                .write_all(&(text.len() as u64).to_le_bytes())
                .and_then(|_| control.write_all(text.as_bytes()))
                .map_err(|e| e.to_string())?;
            let mut reply = String::new();
            std::io::BufReader::new(child.stdout.take().ok_or("Missing clipboard output")?)
                .read_line(&mut reply)
                .map_err(|e| e.to_string())?;
            if reply.trim() == "OK" {
                Ok(())
            } else {
                Err("Cannot access clipboard".into())
            }
        })();
        if result.is_err() {
            let _ = child.kill();
            let _ = child.wait();
            return result;
        }
        let mut active = ACTIVE_HELPER.lock().unwrap_or_else(|e| e.into_inner());
        if session.revoked() {
            let _ = control.write_all(b"C");
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            return Err("Session cleared; copy cancelled".into());
        }
        let helper = Arc::new(Mutex::new(OwnedHelper {
            child,
            control,
            session: session.clone(),
        }));
        if let Some(previous) = active.replace(helper.clone())
            && let Ok(mut previous) = previous.lock()
        {
            let _ = previous.child.kill();
            let _ = previous.child.wait();
        }
        drop(active);
        std::thread::spawn(move || {
            loop {
                if let Ok(mut helper) = helper.lock() {
                    if !matches!(helper.child.try_wait(), Ok(None)) {
                        break;
                    }
                } else {
                    break;
                }
                std::thread::sleep(Duration::from_secs(1));
            }
        });
        Ok(())
    }
}

pub fn run_helper(expires_milliseconds: u64) -> anyhow::Result<()> {
    let lifetime = Duration::from_millis(expires_milliseconds)
        .saturating_sub(SystemTime::now().duration_since(UNIX_EPOCH)?);
    let started = std::time::Instant::now();
    let mut input = std::io::stdin();
    let mut header = [0u8; 8];
    input.read_exact(&mut header)?;
    let size = u64::from_le_bytes(header);
    anyhow::ensure!(size <= 16 * 1024 * 1024, "Clipboard input too large");
    let mut bytes = Zeroizing::new(vec![0u8; size as usize]);
    input.read_exact(&mut bytes)?;
    let secret = std::str::from_utf8(&bytes)?;
    let mut clipboard = Clipboard::new()?;
    anyhow::ensure!(started.elapsed() < lifetime, "Clipboard deadline expired");
    clipboard.set_text(secret.to_string())?;
    println!("OK");
    std::io::stdout().flush()?;
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut signal = [0u8; 1];
        if input.read_exact(&mut signal).is_ok() && signal == [b'C'] {
            let _ = sender.send(());
        }
    });
    finish_clipboard(&mut clipboard, secret, started + lifetime, &receiver)
}

trait ClipboardAccess {
    fn read(&mut self) -> Result<String, arboard::Error>;
    fn clear_value(&mut self) -> Result<(), arboard::Error>;
}
impl ClipboardAccess for Clipboard {
    fn read(&mut self) -> Result<String, arboard::Error> {
        self.get_text()
    }
    fn clear_value(&mut self) -> Result<(), arboard::Error> {
        self.clear()
    }
}

fn finish_clipboard(
    clipboard: &mut impl ClipboardAccess,
    secret: &str,
    deadline: std::time::Instant,
    receiver: &std::sync::mpsc::Receiver<()>,
) -> anyhow::Result<()> {
    if matches!(
        receiver.recv_timeout(deadline.saturating_duration_since(std::time::Instant::now())),
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected)
    ) {
        // Parent exit is not a cancellation. Keep the original expiry.
        std::thread::sleep(deadline.saturating_duration_since(std::time::Instant::now()));
    }
    if clipboard
        .read()
        .is_ok_and(|current| Zeroizing::new(current).as_str() == secret)
    {
        clipboard.clear_value()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct FakeClipboard(String);
    impl ClipboardAccess for FakeClipboard {
        fn read(&mut self) -> Result<String, arboard::Error> {
            Ok(self.0.clone())
        }
        fn clear_value(&mut self) -> Result<(), arboard::Error> {
            self.0.clear();
            Ok(())
        }
    }
    #[test]
    fn cancellation_clears_owned_text_and_preserves_other_content() {
        for value in ["owned secret", "copied elsewhere"] {
            let mut clipboard = FakeClipboard(value.into());
            let (sender, receiver) = std::sync::mpsc::channel();
            sender.send(()).unwrap();
            finish_clipboard(
                &mut clipboard,
                "owned secret",
                std::time::Instant::now() + Duration::from_secs(60),
                &receiver,
            )
            .unwrap();
            assert_eq!(
                clipboard.0,
                if value == "owned secret" { "" } else { value }
            );
        }
    }
    #[test]
    fn parent_exit_waits_for_expiry_instead_of_clearing_early() {
        let mut clipboard = FakeClipboard("owned secret".into());
        let (sender, receiver) = std::sync::mpsc::channel::<()>();
        drop(sender);
        let deadline = std::time::Instant::now() + Duration::from_millis(20);
        finish_clipboard(&mut clipboard, "owned secret", deadline, &receiver).unwrap();
        assert!(std::time::Instant::now() >= deadline);
        assert!(clipboard.0.is_empty());
    }
    #[test]
    fn timeout_clears_without_a_parent_signal() {
        let mut clipboard = FakeClipboard("owned secret".into());
        let (_sender, receiver) = std::sync::mpsc::channel::<()>();
        finish_clipboard(
            &mut clipboard,
            "owned secret",
            std::time::Instant::now(),
            &receiver,
        )
        .unwrap();
        assert!(clipboard.0.is_empty());
    }
    #[test]
    fn revoked_sessions_reject_late_copies_without_starting_helper() {
        let session = ClipboardSession::default();
        session.revoke();
        assert!(
            ClipboardManager::new()
                .copy_for_session("never copy", Duration::from_secs(10), &session)
                .unwrap_err()
                .contains("cancelled")
        );
        assert!(!ClipboardSession::default().revoked());
    }
}
