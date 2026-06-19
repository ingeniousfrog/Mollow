#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
mod portable;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::NativeProbe;
#[cfg(target_os = "macos")]
pub use macos::NativeProbe;
#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
pub use portable::NativeProbe;
#[cfg(target_os = "windows")]
pub use windows::NativeProbe;
