#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(target_os = "macos"))]
mod portable;

#[cfg(target_os = "macos")]
pub use macos::NativeProbe;
#[cfg(not(target_os = "macos"))]
pub use portable::NativeProbe;
