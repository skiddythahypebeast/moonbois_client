use std::{future::Future, path::PathBuf, process::Command};
use console::style;
use spinoff::{spinners, Spinner};
use tokio::{select, task::JoinError};

pub struct Loader<'a> {
    prompt: &'a str
}

impl <'a>Loader<'a> {
    pub fn new() -> Self {
        Self {
            prompt: "Loading"
        }
    }
    pub fn with_prompt(mut self, prompt: &'a str) -> Self {
        self.prompt = prompt;

        self
    }
    pub async fn interact_with_cancel<R>(&self, fut: impl Future<Output = R>) -> Result<Option<R>, LoaderError> {
        let moonbois_root = if std::env::var("CARGO").is_ok() {
            PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
                .join("target")
                .join(std::env::var("PROFILE").unwrap_or_else(|_| "debug".to_string()))
                .display()
                .to_string()
        } else {
            std::env::var("MOONBOIS_ROOT").expect("Missing MOONBOIS_ROOT env variable")
        };

        #[cfg(not(windows))]
        let executable = PathBuf::from(moonbois_root).join("detect_enter");

        #[cfg(windows)]
        let executable = PathBuf::from(moonbois_root).join("detect_enter.exe");

        let mut child = Command::new(executable)
            .arg(self.prompt)
            .spawn()?;
        let process_id = child.id();

        let mut spinner = Spinner::new(
            spinners::Moon, 
            format!("{} {}", self.prompt, style("| hit enter to cancel").dim()), 
            None
        );

        let exit_handle = tokio::spawn(async move {
            let result = child.wait()?;

            if !result.success() {
                return Err(LoaderError::ChildProcessError)
            }

            Ok::<(), LoaderError>(())
        });

        select! {
            result = exit_handle => {
                spinner.clear();
                match result {
                    Ok(Ok(_)) => return Ok(None),
                    Ok(Err(err)) => return Err(err),
                    Err(err) => return Err(LoaderError::from(err))
                }
            },
            result = fut => {
                spinner.clear();

                #[cfg(target_os = "windows")]
                Command::new("taskkill")
                    .arg("/F")
                    .arg("/PID")
                    .arg(process_id.to_string())
                    .output()
                    .expect("Failed to execute taskkill");
            
                #[cfg(not(target_os = "windows"))]
                Command::new("kill")
                    .args(["-9", &process_id.to_string()])
                    .spawn()
                    .expect("Failed to kill process on Unix");

                return Ok(Some(result))
            }
        };
    }
    pub async fn interact<R>(&self, fut: impl Future<Output = R>) -> R {
        let mut spinner = Spinner::new(
            spinners::Moon, 
            format!("{}", self.prompt), 
            None
        );

        let result = fut.await;

        spinner.clear();

        result
    }
}

#[derive(thiserror::Error, Debug)]
pub enum LoaderError {
    #[error("An error occured in the loader child process")]
    ChildProcessError,
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Join error: {0}")]
    JoinError(#[from] JoinError)
}