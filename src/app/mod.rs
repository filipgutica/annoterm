//! Application state and commands, independent from terminal input and storage.

mod command;
mod state;

pub use command::Command;
pub use state::{App, Comment, Mode};
