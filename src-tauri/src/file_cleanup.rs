//! Delete temporary files only after their owners have closed them. Windows
//! sharing/lock violations can outlive a close (for example during scanning),
//! so retry those errors briefly without hiding persistent failures.
use std::{io, path::Path, time::Duration};
use tokio::time::{sleep, Instant};

const CLEANUP_TIMEOUT: Duration = Duration::from_secs(2);

async fn retry<T>(
    mut operation: impl FnMut() -> io::Result<T>,
    windows: bool,
    timeout: Duration,
) -> io::Result<T> {
    let deadline = Instant::now() + timeout;
    let mut delay = Duration::from_millis(10);
    loop {
        match operation() {
            Err(error)
                if windows
                    && matches!(error.raw_os_error(), Some(32 | 33))
                    && Instant::now() < deadline =>
            {
                sleep(delay.min(deadline.saturating_duration_since(Instant::now()))).await;
                delay = (delay * 2).min(Duration::from_millis(100));
            }
            result => return result,
        }
    }
}

pub(crate) async fn remove_file(path: &Path) -> io::Result<()> {
    retry(
        || std::fs::remove_file(path),
        cfg!(windows),
        CLEANUP_TIMEOUT,
    )
    .await
}

pub(crate) async fn remove_dir_all(path: &Path) -> io::Result<()> {
    retry(
        || std::fs::remove_dir_all(path),
        cfg!(windows),
        CLEANUP_TIMEOUT,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn temporary_windows_sharing_and_lock_violations_are_retried() {
        for code in [32, 33] {
            let mut attempts = 0;
            retry(
                || {
                    attempts += 1;
                    if attempts < 3 {
                        Err(io::Error::from_raw_os_error(code))
                    } else {
                        Ok(())
                    }
                },
                true,
                Duration::from_secs(1),
            )
            .await
            .unwrap();
            assert_eq!(attempts, 3);
        }
    }

    #[tokio::test]
    async fn unrelated_errors_and_non_windows_errors_are_returned_immediately() {
        for (windows, code) in [(true, 5), (true, 2), (false, 32), (false, 33)] {
            let mut attempts = 0;
            let error = retry::<()>(
                || {
                    attempts += 1;
                    Err(io::Error::from_raw_os_error(code))
                },
                windows,
                Duration::from_secs(1),
            )
            .await
            .unwrap_err();
            assert_eq!(error.raw_os_error(), Some(code));
            assert_eq!(attempts, 1);
        }
    }

    #[tokio::test]
    async fn permanent_windows_locks_fail_with_the_original_error_after_timeout() {
        let start = Instant::now();
        let error = retry::<()>(
            || Err(io::Error::from_raw_os_error(32)),
            true,
            Duration::from_millis(30),
        )
        .await
        .unwrap_err();
        assert_eq!(error.raw_os_error(), Some(32));
        assert!(start.elapsed() >= Duration::from_millis(30));
    }

    #[tokio::test]
    async fn cleanup_really_removes_files_and_directories_and_reports_missing_files() {
        let directory = std::env::temp_dir().join(crate::ids::new_id());
        std::fs::create_dir(&directory).unwrap();
        let file = directory.join("temporary.db");
        std::fs::write(&file, b"temporary fixture").unwrap();
        remove_file(&file).await.unwrap();
        assert!(!file.exists());
        assert_eq!(
            remove_file(&file).await.unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
        remove_dir_all(&directory).await.unwrap();
        assert!(!directory.exists());
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn cleanup_waits_for_a_real_windows_file_lock_to_be_released() {
        use std::os::windows::fs::OpenOptionsExt;
        let directory = std::env::temp_dir().join(crate::ids::new_id());
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("locked.db");
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .share_mode(0)
            .open(&path)
            .unwrap();
        assert_eq!(
            std::fs::remove_file(&path).unwrap_err().raw_os_error(),
            Some(32)
        );
        let removal = tokio::spawn(async move { remove_file(&path).await });
        sleep(Duration::from_millis(30)).await;
        assert!(!removal.is_finished());
        drop(file);
        removal.await.unwrap().unwrap();
        remove_dir_all(&directory).await.unwrap();
    }
}
