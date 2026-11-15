//! Shared logic for the Creed shell, CLI and helper.

pub mod color;
pub mod config;
pub mod hardware;
pub mod hypr;
pub mod mode;
pub mod modules;
pub mod palette;
pub mod paths;
pub mod template;
pub mod theme;
pub mod tweak;

pub use color::Color;
pub use config::Config;
pub use paths::Paths;
