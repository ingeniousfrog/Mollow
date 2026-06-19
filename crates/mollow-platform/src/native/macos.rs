use std::ffi::{CString, c_char, c_int, c_void};
use std::io;
use std::mem::size_of;
use std::ptr;

use mollow_core::{CpuInfo, DataSource, MemoryInfo, SystemInfo};

use crate::{PlatformProbe, ProbeError};

unsafe extern "C" {
    fn sysctlbyname(
        name: *const c_char,
        old_value: *mut c_void,
        old_length: *mut usize,
        new_value: *mut c_void,
        new_length: usize,
    ) -> c_int;
}

pub struct NativeProbe;

impl PlatformProbe for NativeProbe {
    fn system(&self) -> Result<SystemInfo, ProbeError> {
        let read = |name| {
            sysctl_string(name).map_err(|error| ProbeError::new("system", error.to_string()))
        };

        Ok(SystemInfo {
            os_name: "macOS".to_owned(),
            os_version: Some(read("kern.osproductversion")?),
            kernel_version: Some(read("kern.osrelease")?),
            architecture: std::env::consts::ARCH.to_owned(),
            hostname: Some(read("kern.hostname")?),
        })
    }

    fn cpu(&self) -> Result<CpuInfo, ProbeError> {
        let logical_cores = sysctl_u32("hw.logicalcpu")
            .or_else(|_| {
                u32::try_from(
                    std::thread::available_parallelism()
                        .map_err(|error| io::Error::other(error.to_string()))?
                        .get(),
                )
                .map_err(|error| io::Error::other(error.to_string()))
            })
            .map_err(|error| ProbeError::new("cpu", error.to_string()))?;
        let model = sysctl_string("machdep.cpu.brand_string")
            .or_else(|_| sysctl_string("hw.model"))
            .map_err(|error| ProbeError::new("cpu", error.to_string()))?;
        let physical_cores = sysctl_u32("hw.physicalcpu")
            .map_err(|error| ProbeError::new("cpu", error.to_string()))?;

        Ok(CpuInfo {
            model: Some(model),
            physical_cores: Some(physical_cores),
            logical_cores,
        })
    }

    fn memory(&self) -> Result<MemoryInfo, ProbeError> {
        let total_bytes = sysctl_u64("hw.memsize")
            .map_err(|error| ProbeError::new("memory", error.to_string()))?;

        Ok(MemoryInfo {
            total_bytes,
            available_bytes: None,
        })
    }

    fn source(&self) -> DataSource {
        DataSource {
            provider: "macos-sysctl".to_owned(),
            detail: Some("sysctlbyname FFI".to_owned()),
        }
    }
}

fn sysctl_string(name: &str) -> io::Result<String> {
    let name = c_name(name)?;
    let mut length = 0_usize;

    // SAFETY: `name` is a valid NUL-terminated C string and the first call only writes `length`.
    let status = unsafe {
        sysctlbyname(
            name.as_ptr(),
            ptr::null_mut(),
            &raw mut length,
            ptr::null_mut(),
            0,
        )
    };
    if status != 0 {
        return Err(io::Error::last_os_error());
    }

    let mut bytes = vec![0_u8; length];
    // SAFETY: `bytes` owns `length` writable bytes and all pointers remain valid for the call.
    let status = unsafe {
        sysctlbyname(
            name.as_ptr(),
            bytes.as_mut_ptr().cast::<c_void>(),
            &raw mut length,
            ptr::null_mut(),
            0,
        )
    };
    if status != 0 {
        return Err(io::Error::last_os_error());
    }

    bytes.truncate(length);
    if bytes.last() == Some(&0) {
        bytes.pop();
    }

    String::from_utf8(bytes).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn sysctl_u32(name: &str) -> io::Result<u32> {
    let mut value = 0_u32;
    read_sysctl_number(name, &raw mut value)?;
    Ok(value)
}

fn sysctl_u64(name: &str) -> io::Result<u64> {
    let mut value = 0_u64;
    read_sysctl_number(name, &raw mut value)?;
    Ok(value)
}

fn read_sysctl_number<T>(name: &str, value: *mut T) -> io::Result<()> {
    let name = c_name(name)?;
    let mut length = size_of::<T>();

    // SAFETY: `value` points to initialized writable storage of `size_of::<T>()` bytes.
    let status = unsafe {
        sysctlbyname(
            name.as_ptr(),
            value.cast::<c_void>(),
            &raw mut length,
            ptr::null_mut(),
            0,
        )
    };
    if status != 0 {
        return Err(io::Error::last_os_error());
    }
    if length != size_of::<T>() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unexpected sysctl value size: {length}"),
        ));
    }

    Ok(())
}

fn c_name(name: &str) -> io::Result<CString> {
    CString::new(name).map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))
}
