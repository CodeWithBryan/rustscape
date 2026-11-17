//! Client device presentation around the shared original PCM numeric core.
pub use rs09_audio::pcm::{Mixer, Voice, add_music, clip, wav};
#[cfg(target_arch = "wasm32")]
pub mod browser;
pub mod feed;
pub mod presentation;
#[cfg(target_os = "windows")]
pub mod windows;
#[cfg(target_arch = "wasm32")]
pub use browser::{Handle, Output};
#[cfg(target_os = "windows")]
pub use windows::{Handle, Output};
