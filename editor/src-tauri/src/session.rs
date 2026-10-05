//! Tauri-facing lifecycle adapter for the shared native session owner.

use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Mutex, MutexGuard};
use std::thread::JoinHandle;

use vactr::cli::args::HostChoice;
#[cfg(target_os = "ios")]
use vactr::cli::owner::AudioSessionEvent;
use vactr::cli::owner::SessionOwner;

/// Lazily owns one native session and tracks IPC forwarding threads.
pub struct SessionBridge {
    owner: Mutex<Option<SessionOwner>>,
    threads: Mutex<Vec<JoinHandle<()>>>,
    host: HostChoice,
    cwd: PathBuf,
}

impl SessionBridge {
    /// Create a bridge. The owner starts on the first connection.
    #[must_use]
    pub fn new(host: HostChoice, cwd: PathBuf) -> Self {
        Self {
            owner: Mutex::new(None),
            threads: Mutex::new(Vec::new()),
            host,
            cwd,
        }
    }

    /// Connect an outbound sender, starting the shared owner if needed.
    pub fn connect(&self, outbound: Sender<String>) -> Result<u32, String> {
        let mut owner = self.lock_owner()?;
        if owner.is_none() {
            *owner = Some(SessionOwner::spawn(self.host, self.cwd.clone())?);
        }
        let active_owner = owner
            .as_ref()
            .ok_or_else(|| "session owner was not initialized".to_string())?;
        Ok(active_owner.connect(outbound))
    }

    /// Submit one protocol message if the owner is active.
    pub fn send(&self, id: u32, text: String) -> Result<(), String> {
        if let Some(owner) = self.lock_owner()?.as_ref() {
            owner.send(id, text);
        }
        Ok(())
    }

    /// Close one connection if the owner is active.
    pub fn close(&self, id: u32) -> Result<(), String> {
        if let Some(owner) = self.lock_owner()?.as_ref() {
            owner.close(id);
        }
        Ok(())
    }

    /// Forward a session's routed messages to a Tauri IPC channel.
    pub fn retain_forwarder(&self, thread: JoinHandle<()>) -> Result<(), String> {
        self.lock_threads()?.push(thread);
        Ok(())
    }

    /// Notify an active owner about an iOS audio-session lifecycle event.
    #[cfg(target_os = "ios")]
    pub fn audio_event(&self, event: AudioSessionEvent) -> Result<(), String> {
        if let Some(owner) = self.lock_owner()?.as_ref() {
            owner.audio_event(event);
        }
        Ok(())
    }

    /// Stop the native owner before joining IPC forwarders.
    pub fn shutdown(&self) -> Result<(), String> {
        if let Some(owner) = self.lock_owner()?.take() {
            owner.shutdown();
        }
        for thread in self.lock_threads()?.drain(..) {
            let _ = thread.join();
        }
        Ok(())
    }

    fn lock_owner(&self) -> Result<MutexGuard<'_, Option<SessionOwner>>, String> {
        self.owner
            .lock()
            .map_err(|_| "session owner lock is poisoned".to_string())
    }

    fn lock_threads(&self) -> Result<MutexGuard<'_, Vec<JoinHandle<()>>>, String> {
        self.threads
            .lock()
            .map_err(|_| "session forwarding thread lock is poisoned".to_string())
    }
}

impl Drop for SessionBridge {
    fn drop(&mut self) {
        if let Ok(owner) = self.owner.get_mut() {
            if let Some(owner) = owner.take() {
                owner.shutdown();
            }
        }
        if let Ok(threads) = self.threads.get_mut() {
            for thread in threads.drain(..) {
                let _ = thread.join();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::mpsc;
    use std::time::Duration;

    use vactr::cli::args::HostChoice;

    use super::SessionBridge;

    fn bridge() -> (SessionBridge, PathBuf) {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let cwd =
            std::env::temp_dir().join(format!("vactr-tauri-session-{}-{id}", std::process::id()));
        std::fs::create_dir_all(&cwd).expect("create test session directory");
        (SessionBridge::new(HostChoice::Noop, cwd.clone()), cwd)
    }

    #[test]
    fn routes_clock_probe_reply_to_sender() {
        let (bridge, cwd) = bridge();
        let (outbound, replies) = mpsc::channel();
        let id = bridge.connect(outbound).expect("connect session");
        bridge
            .send(
                id,
                r#"{"v":1,"seq":9,"kind":"clock-probe","body":{"page_send":12.5}}"#.to_string(),
            )
            .expect("send clock probe");

        let reply = replies
            .recv_timeout(Duration::from_secs(2))
            .expect("clock-probe reply");
        assert!(reply.contains("\"kind\":\"clock-probe\""));
        assert!(reply.contains("\"re\":9"));
        assert!(reply.contains("\"page_send\":12.5"));

        bridge.shutdown().expect("shutdown bridge");
        std::fs::remove_dir_all(cwd).expect("remove test session directory");
    }

    #[test]
    fn ignores_sends_after_connection_close() {
        let (bridge, cwd) = bridge();
        let (outbound, replies) = mpsc::channel();
        let id = bridge.connect(outbound).expect("connect session");
        bridge.close(id).expect("close connection");
        bridge
            .send(
                id,
                r#"{"v":1,"seq":9,"kind":"clock-probe","body":{"page_send":12.5}}"#.to_string(),
            )
            .expect("late send is ignored");

        assert!(replies.recv_timeout(Duration::from_millis(100)).is_err());
        bridge.shutdown().expect("shutdown bridge");
        std::fs::remove_dir_all(cwd).expect("remove test session directory");
    }
}
