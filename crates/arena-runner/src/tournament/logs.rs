//! Continuously drain child output, retaining a bounded prefix and explicit
//! byte counts. File failures are returned after draining, never hidden.
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::thread::JoinHandle;

pub const LOG_LIMIT: usize = 1024 * 1024;

pub struct Capture {
    handle: Option<JoinHandle<io::Result<Vec<u8>>>>,
}
impl Capture {
    pub fn start(mut input: impl Read + Send + 'static, path: PathBuf) -> Self {
        let handle = std::thread::spawn(move || {
            let mut output = std::fs::File::create(&path);
            let mut saved = Vec::new();
            let mut total = 0u64;
            let mut buf = [0u8; 8192];
            loop {
                let count = match input.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => n,
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(e),
                };
                total += count as u64;
                let keep = count.min(LOG_LIMIT - saved.len());
                saved.extend_from_slice(&buf[..keep]);
                if let Ok(file) = &mut output {
                    if let Err(e) = file.write_all(&buf[..keep]) {
                        output = Err(e);
                    }
                }
            }
            let file = output?;
            file.sync_all()?;
            super::durable::atomic_write(
                &PathBuf::from(format!("{}.meta.json", path.display())),
                format!(
                    "{{\"bytes_seen\":{total},\"bytes_saved\":{},\"truncated\":{}}}\n",
                    saved.len(),
                    total > saved.len() as u64
                )
                .as_bytes(),
            )?;
            Ok(saved)
        });
        Self {
            handle: Some(handle),
        }
    }
    pub fn finish(mut self) -> io::Result<Vec<u8>> {
        self.handle
            .take()
            .unwrap()
            .join()
            .map_err(|_| io::Error::other("log collector panicked"))?
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
