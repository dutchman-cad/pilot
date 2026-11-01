//! Single in-flight remote exchanges with complete-operation deadlines.
use super::protocol;
use std::{
    io::{Read, Write},
    net::{Shutdown, SocketAddr, TcpStream},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver, SyncSender},
    thread,
    time::{Duration, Instant},
};
static DIRECTORY_SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
#[derive(Clone, Debug)]
pub enum Endpoint {
    Tcp(SocketAddr),
    Program { path: PathBuf, headless: bool },
}
pub struct RemoteSession {
    endpoint: Endpoint,
    worker: Option<Worker>,
    pub commands: usize,
    pub(crate) command_limit: usize,
    pub(crate) deadline: Instant,
    directory: Option<PathBuf>,
}
struct Worker {
    tx: SyncSender<String>,
    written: Receiver<Result<(), String>>,
    rx: Receiver<Result<String, String>>,
    socket: Option<TcpStream>,
    child: Option<Child>,
    headless: bool,
}
fn exchange_worker(w: &Worker, command: &str, deadline: Instant) -> Result<String, String> {
    w.tx.send(command.into())
        .map_err(|_| "remote worker lost".to_owned())?;
    w.written
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .map_err(|e| format!("remote write deadline: {e}"))??;
    w.rx.recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .map_err(|e| format!("remote deadline or EOF: {e}"))?
}
fn worker(input: impl Read + Send + 'static, output: impl Write + Send + 'static) -> Worker {
    let (tx, requests) = mpsc::sync_channel::<String>(1);
    let (replies, rx) = mpsc::sync_channel(1);
    let (writes, written) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let mut output = output;
        while let Ok(command) = requests.recv() {
            let result = protocol::write(&mut output, &command);
            let failed = result.is_err();
            if writes.send(result).is_err() || failed {
                break;
            }
        }
    });
    thread::spawn(move || {
        let mut input = input;
        loop {
            let result = protocol::read(&mut input);
            let failed = result.is_err();
            if replies.send(result).is_err() || failed {
                break;
            }
        }
    });
    Worker {
        tx,
        written,
        rx,
        socket: None,
        child: None,
        headless: false,
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        if let Some(socket) = &self.socket {
            let _ = socket.shutdown(Shutdown::Both);
        }
        // Disconnect the request channel before waiting, so idle pipe workers close stdin.
        let (dummy, _) = mpsc::sync_channel(1);
        let old = std::mem::replace(&mut self.tx, dummy);
        drop(old);
        if let Some(child) = &mut self.child {
            let deadline = Instant::now() + Duration::from_secs(2);
            while Instant::now() < deadline {
                if matches!(child.try_wait(), Ok(Some(_))) {
                    return;
                }
                thread::sleep(Duration::from_millis(20));
            }
            if self.headless {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}
impl RemoteSession {
    pub fn new(endpoint: Endpoint) -> Self {
        Self {
            endpoint,
            worker: None,
            commands: 0,
            command_limit: 160,
            deadline: Instant::now() + Duration::from_secs(900),
            directory: None,
        }
    }
    fn connect(&mut self) -> Result<(), String> {
        let w = match &self.endpoint {
            Endpoint::Tcp(address) => {
                let mut result = Err("connection not attempted".into());
                let deadline = (Instant::now() + Duration::from_secs(3)).min(self.deadline);
                for delay in [0, 250, 500] {
                    if self.commands >= self.command_limit {
                        return Err("SolidCraft 98 command/reconnect budget exhausted".into());
                    }
                    self.commands += 1;
                    let pause = Duration::from_millis(delay);
                    if deadline.saturating_duration_since(Instant::now()) <= pause {
                        break;
                    }
                    thread::sleep(pause);
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        break;
                    }
                    match TcpStream::connect_timeout(address, remaining) {
                        Ok(stream) => {
                            // No socket read timeout: the reader waits for the next answer while
                            // the model thinks, which can take longer than any command. exchange()
                            // bounds each command at fifteen seconds, and dropping the worker shuts
                            // the socket down.
                            stream
                                .set_write_timeout(Some(Duration::from_secs(15)))
                                .map_err(|e| e.to_string())?;
                            let input = stream.try_clone().map_err(|e| e.to_string())?;
                            let socket = stream.try_clone().map_err(|e| e.to_string())?;
                            let mut w = worker(input, stream);
                            w.socket = Some(socket);
                            result = Ok(w);
                            break;
                        }
                        Err(e) => result = Err(format!("cannot connect to SolidCraft 98: {e}")),
                    }
                }
                result?
            }
            Endpoint::Program { path, headless } => {
                if self.directory.is_some() {
                    return Err("SolidCraft 98 process lost; start an explicit new session".into());
                }
                let dir = std::env::temp_dir().join(format!(
                    "pilot-solidcraft98-{}-{}-{}",
                    std::process::id(),
                    crate::audit::now(),
                    DIRECTORY_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                ));
                std::fs::create_dir(&dir)
                    .map_err(|e| format!("cannot create isolated application directory: {e}"))?;
                self.directory = Some(dir.clone());
                let path = std::fs::canonicalize(path)
                    .map_err(|e| format!("cannot resolve SolidCraft 98 program path: {e}"))?;
                std::fs::create_dir(dir.join("Documents")).map_err(|e| e.to_string())?;
                let mut cmd = Command::new(path);
                cmd.arg("--remote");
                if *headless {
                    cmd.arg("--headless");
                }
                let mut child = cmd
                    .current_dir(&dir)
                    .env("HOME", &dir)
                    .env("USERPROFILE", &dir)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .map_err(|e| format!("cannot start SolidCraft 98: {e}"))?;
                let stderr = child.stderr.take().ok_or("child stderr unavailable")?;
                thread::spawn(move || {
                    let mut stderr = stderr;
                    let mut buf = [0; 4096];
                    while matches!(stderr.read(&mut buf),Ok(n) if n>0) {}
                });
                let input = child.stdout.take().ok_or("child stdout unavailable")?;
                let output = child.stdin.take().ok_or("child stdin unavailable")?;
                let mut w = worker(input, output);
                w.child = Some(child);
                w.headless = *headless;
                w
            }
        };
        self.worker = Some(w);
        Ok(())
    }
    pub fn exchange(&mut self, command: &str) -> Result<String, String> {
        protocol::encode(command)?;
        if self.commands >= self.command_limit || Instant::now() >= self.deadline {
            return Err(
                "SolidCraft 98 session budget exhausted; start an explicit new session".into(),
            );
        }
        if self.worker.is_none() {
            self.connect()?;
        }
        if self.commands >= self.command_limit {
            return Err("SolidCraft 98 command/reconnect budget exhausted".into());
        }
        self.commands += 1;
        let w = self.worker.as_ref().ok_or("remote worker unavailable")?;
        let deadline = (Instant::now() + Duration::from_secs(15)).min(self.deadline);
        let result = exchange_worker(w, command, deadline);
        if result.is_err() {
            self.worker.take();
            return Err(format!(
                "Outcome unknown after write started; command was not replayed. {}",
                result.err().unwrap_or_default()
            ));
        }
        result
    }
    pub fn is_tcp(&self) -> bool {
        matches!(self.endpoint, Endpoint::Tcp(_))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    struct Blocked(Receiver<()>);
    impl Write for Blocked {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            let _ = self.0.recv();
            Err(std::io::Error::other("test writer released"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    #[test]
    fn tcp_session_survives_a_long_model_turn() {
        use std::io::{BufRead, BufReader};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let peer = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut out = stream.try_clone().unwrap();
            for line in BufReader::new(stream).lines() {
                if line.unwrap() == "look" {
                    out.write_all(b"screen\n.\n").unwrap();
                }
            }
        });
        let mut session = RemoteSession::new(Endpoint::Tcp(address));
        assert_eq!(session.exchange("look").unwrap(), "screen\n");
        // Longer than the fifteen-second command deadline, as a slow model turn can be.
        thread::sleep(Duration::from_secs(16));
        assert_eq!(session.exchange("look").unwrap(), "screen\n");
        drop(session);
        peer.join().unwrap();
    }
    #[test]
    fn complete_deadline_includes_blocked_writes() {
        let (release, blocked) = mpsc::sync_channel(1);
        let w = worker(std::io::Cursor::new(b"ok\n.\n"), Blocked(blocked));
        let result = exchange_worker(&w, "look", Instant::now() + Duration::from_millis(30));
        assert!(result.unwrap_err().contains("write deadline"));
        release.send(()).unwrap();
    }
}
