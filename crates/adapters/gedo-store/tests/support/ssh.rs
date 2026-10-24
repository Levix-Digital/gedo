//! A throwaway `sshd` on localhost with generated keys, for the SSH remote test (Linux).

use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const STARTUP_TIMEOUT_MS: u64 = 10_000;

pub struct Server {
    process: Child,
    port: u16,
    user: String,
    client_key: PathBuf,
}

fn generate_key(path: &Path) {
    let status = Command::new("ssh-keygen")
        .args(["-q", "-t", "ed25519", "-N", "", "-f"])
        .arg(path)
        .status()
        .expect("ssh-keygen runs");
    assert!(status.success(), "ssh-keygen failed");
}

fn find_sshd() -> PathBuf {
    ["/usr/sbin/sshd", "/usr/bin/sshd", "/sbin/sshd"]
        .iter()
        .map(PathBuf::from)
        .find(|path| path.exists())
        .expect("sshd installed (openssh-server)")
}

impl Server {
    pub fn start(directory: &Path) -> Self {
        let directory = directory.join("ssh");
        std::fs::create_dir_all(&directory).expect("directory");
        let host_key = directory.join("host_key");
        let client_key = directory.join("client_key");
        generate_key(&host_key);
        generate_key(&client_key);
        let authorized = directory.join("authorized_keys");
        std::fs::copy(client_key.with_extension("pub"), &authorized).expect("authorized keys");

        let port = TcpListener::bind("127.0.0.1:0")
            .expect("free port")
            .local_addr()
            .expect("address")
            .port();
        let user = std::env::var("USER")
            .or_else(|_| std::env::var("LOGNAME"))
            .unwrap_or_else(|_| {
                String::from_utf8(Command::new("id").arg("-un").output().expect("id runs").stdout)
                    .expect("utf-8")
                    .trim()
                    .to_owned()
            });
        let config = directory.join("sshd_config");
        std::fs::write(
            &config,
            format!(
                "Port {port}\nListenAddress 127.0.0.1\nHostKey {}\nAuthorizedKeysFile {}\nPidFile {}\n\
                 StrictModes no\nUsePAM no\nPasswordAuthentication no\nKbdInteractiveAuthentication no\n\
                 PubkeyAuthentication yes\nPermitRootLogin yes\nAllowUsers {user}\n",
                host_key.display(),
                authorized.display(),
                directory.join("sshd.pid").display(),
            ),
        )
        .expect("sshd config");
        let process = Command::new(find_sshd())
            .arg("-D")
            .arg("-e")
            .arg("-f")
            .arg(&config)
            .stdout(Stdio::null())
            .stderr(std::fs::File::create(directory.join("sshd.log")).expect("log file"))
            .spawn()
            .expect("sshd starts");

        let deadline = Instant::now() + Duration::from_millis(STARTUP_TIMEOUT_MS);
        while TcpStream::connect(("127.0.0.1", port)).is_err() {
            let log = std::fs::read_to_string(directory.join("sshd.log")).unwrap_or_default();
            assert!(Instant::now() < deadline, "sshd did not start listening: {log}");
            std::thread::sleep(Duration::from_millis(50));
        }
        Server {
            process,
            port,
            user,
            client_key,
        }
    }

    /// `GIT_SSH_COMMAND` reaching this server with the generated key.
    pub fn ssh_command(&self) -> String {
        format!(
            "ssh -i {} -o IdentitiesOnly=yes -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR",
            self.client_key.display()
        )
    }

    /// The SSH URL of the repository at `path` on this machine.
    pub fn url(&self, path: &Path) -> String {
        format!("ssh://{}@127.0.0.1:{}{}", self.user, self.port, path.display())
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if self.process.kill().is_ok() {
            self.process.wait().ok();
        }
    }
}
