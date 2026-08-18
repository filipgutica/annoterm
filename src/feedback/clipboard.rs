use std::{
    io::Write,
    process::{Command, Stdio},
};

use base64::{Engine, engine::general_purpose::STANDARD};

const OSC52_MAX_PAYLOAD_BYTES: usize = 100_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClipboardBackend {
    Pbcopy,
    WlCopy,
    Xclip,
    WindowsClipboard,
    Osc52,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClipboardStatus {
    Copied {
        backend: ClipboardBackend,
    },
    SentOsc52 {
        payload_bytes: usize,
    },
    Unavailable,
    Failed {
        backend: ClipboardBackend,
        message: String,
    },
}

#[derive(Clone, Debug)]
pub struct ClipboardCommand {
    backend: ClipboardBackend,
    executable: String,
    arguments: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Clipboard {
    commands: Vec<ClipboardCommand>,
    allow_osc52: bool,
}

impl Default for Clipboard {
    fn default() -> Self {
        #[cfg(target_os = "macos")]
        let commands = vec![command(ClipboardBackend::Pbcopy, "pbcopy", &[])];
        #[cfg(target_os = "windows")]
        let commands = vec![command(ClipboardBackend::WindowsClipboard, "clip.exe", &[])];
        #[cfg(target_os = "linux")]
        let commands = {
            let mut commands = Vec::new();
            if std::env::var_os("WAYLAND_DISPLAY").is_some() {
                commands.push(command(ClipboardBackend::WlCopy, "wl-copy", &[]));
            }
            if std::env::var_os("DISPLAY").is_some() {
                commands.push(command(
                    ClipboardBackend::Xclip,
                    "xclip",
                    &["-selection", "clipboard"],
                ));
            }
            commands
        };
        Self {
            commands,
            allow_osc52: true,
        }
    }
}

impl Clipboard {
    pub fn with_commands<const N: usize>(commands: [ClipboardCommand; N]) -> Self {
        Self {
            commands: commands.into(),
            allow_osc52: false,
        }
    }

    pub fn copy(&self, content: &str) -> ClipboardStatus {
        let mut last_failure = None;
        for command in &self.commands {
            match run_command(command, content) {
                Ok(()) => {
                    return ClipboardStatus::Copied {
                        backend: command.backend,
                    };
                }
                Err(message) => {
                    last_failure = Some(ClipboardStatus::Failed {
                        backend: command.backend,
                        message,
                    })
                }
            }
        }

        if self.allow_osc52 {
            return copy_with_osc52(content);
        }
        last_failure.unwrap_or(ClipboardStatus::Unavailable)
    }
}

pub fn copy_prompt(prompt: &str) -> ClipboardStatus {
    Clipboard::default().copy(prompt)
}

fn command(backend: ClipboardBackend, executable: &str, arguments: &[&str]) -> ClipboardCommand {
    ClipboardCommand {
        backend,
        executable: executable.into(),
        arguments: arguments.iter().map(ToString::to_string).collect(),
    }
}

fn run_command(command: &ClipboardCommand, content: &str) -> Result<(), String> {
    let mut process = Command::new(&command.executable)
        .args(&command.arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| error.to_string())?;
    process
        .stdin
        .take()
        .ok_or_else(|| "clipboard command did not provide stdin".to_owned())?
        .write_all(content.as_bytes())
        .map_err(|error| error.to_string())?;
    let output = process
        .wait_with_output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        Err(if stderr.is_empty() {
            format!("clipboard command exited with {}", output.status)
        } else {
            stderr
        })
    }
}

fn copy_with_osc52(content: &str) -> ClipboardStatus {
    if content.len() > OSC52_MAX_PAYLOAD_BYTES {
        return ClipboardStatus::Failed {
            backend: ClipboardBackend::Osc52,
            message: format!("OSC 52 payload exceeds {OSC52_MAX_PAYLOAD_BYTES} bytes"),
        };
    }
    let encoded = STANDARD.encode(content.as_bytes());
    let sequence = format!("\x1b]52;c;{encoded}\x07");
    match std::io::stdout()
        .write_all(sequence.as_bytes())
        .and_then(|()| std::io::stdout().flush())
    {
        Ok(()) => ClipboardStatus::SentOsc52 {
            payload_bytes: content.len(),
        },
        Err(error) => ClipboardStatus::Failed {
            backend: ClipboardBackend::Osc52,
            message: error.to_string(),
        },
    }
}
