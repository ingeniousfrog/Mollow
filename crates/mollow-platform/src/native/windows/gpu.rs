use std::ffi::c_void;
use std::io;
use std::ptr;

use mollow_core::GpuInfo;

const ERROR_SUCCESS: i32 = 0;

#[repr(C)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

#[repr(C)]
struct DxgiAdapterDesc {
    description: [u16; 128],
    vendor_id: u32,
    device_id: u32,
    sub_sys_id: u32,
    revision: u32,
    dedicated_video_memory: usize,
    shared_system_memory: usize,
    adapter_luid: [u32; 2],
}

static IID_IDXGIFACTORY: Guid = Guid {
    data1: 0x7b71_66ec,
    data2: 0x21c7,
    data3: 0x44ae,
    data4: [0xb2, 0x1a, 0xc9, 0xae, 0x32, 0x1a, 0xe3, 0x69],
};

#[link(name = "dxgi")]
unsafe extern "system" {
    fn CreateDXGIFactory(riid: *const Guid, factory: *mut *mut c_void) -> i32;
}

type ReleaseFn = unsafe extern "system" fn(*mut c_void) -> u32;
type EnumAdaptersFn = unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> i32;
type GetDescFn = unsafe extern "system" fn(*mut c_void, *mut DxgiAdapterDesc) -> i32;

pub(crate) fn enumerate_gpus() -> io::Result<Vec<GpuInfo>> {
    // SAFETY: DXGI factory creation follows the documented COM contract.
    unsafe {
        let mut factory = ptr::null_mut();
        if CreateDXGIFactory(&raw const IID_IDXGIFACTORY, &raw mut factory) != ERROR_SUCCESS {
            return Err(io::Error::last_os_error());
        }
        if factory.is_null() {
            return Err(io::Error::other("DXGI factory pointer was null"));
        }

        let result = enumerate_from_factory(factory);
        release_com_object(factory);
        result
    }
}

unsafe fn enumerate_from_factory(factory: *mut c_void) -> io::Result<Vec<GpuInfo>> {
    // SAFETY: DXGI factory methods are invoked through valid COM pointers.
    unsafe {
        let vtable = *(factory.cast::<*const usize>());
        let enum_adapters: EnumAdaptersFn = std::mem::transmute(*vtable.add(3));
        let mut gpus = Vec::new();
        let mut index = 0_u32;

        loop {
            let mut adapter = ptr::null_mut();
            let status = enum_adapters(factory, index, &raw mut adapter);
            if status != ERROR_SUCCESS {
                break;
            }
            if adapter.is_null() {
                break;
            }

            if let Ok(gpu) = adapter_description(adapter) {
                gpus.push(gpu);
            }
            release_com_object(adapter);
            index = index
                .checked_add(1)
                .ok_or_else(|| io::Error::other("DXGI adapter index overflowed"))?;
        }

        if gpus.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "DXGI reported no display adapters",
            ));
        }
        Ok(gpus)
    }
}

unsafe fn adapter_description(adapter: *mut c_void) -> io::Result<GpuInfo> {
    // SAFETY: DXGI adapter description is read through a valid COM pointer.
    unsafe {
        let vtable = *(adapter.cast::<*const usize>());
        let get_desc: GetDescFn = std::mem::transmute(*vtable.add(8));
        let mut description = DxgiAdapterDesc {
            description: [0; 128],
            vendor_id: 0,
            device_id: 0,
            sub_sys_id: 0,
            revision: 0,
            dedicated_video_memory: 0,
            shared_system_memory: 0,
            adapter_luid: [0, 0],
        };
        if get_desc(adapter, &raw mut description) != ERROR_SUCCESS {
            return Err(io::Error::last_os_error());
        }

        let name = String::from_utf16_lossy(
            description
                .description
                .split(|unit| *unit == 0)
                .next()
                .unwrap_or_default(),
        )
        .trim()
        .to_owned();

        Ok(GpuInfo {
            name,
            vendor: Some(vendor_name(description.vendor_id)),
            driver_version: None,
            memory_bytes: u64::try_from(description.dedicated_video_memory).ok(),
            apis: vec!["DXGI".to_owned()],
        })
    }
}

unsafe fn release_com_object(object: *mut c_void) {
    // SAFETY: COM release decrements the object reference count.
    unsafe {
        if object.is_null() {
            return;
        }
        let vtable = *(object.cast::<*const usize>());
        let release: ReleaseFn = std::mem::transmute(*vtable.add(2));
        let _ = release(object);
    }
}

fn vendor_name(vendor_id: u32) -> String {
    match vendor_id {
        0x1002 => "AMD".to_owned(),
        0x10de => "NVIDIA".to_owned(),
        0x8086 => "Intel".to_owned(),
        0x106b => "Apple".to_owned(),
        other => format!("PCI {other:04x}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vendor_name_maps_known_ids() {
        assert_eq!(vendor_name(0x8086), "Intel");
    }

    #[test]
    fn adapter_desc_structure_size_matches_dxgi() {
        // DXGI_ADAPTER_DESC: WCHAR Description[128] + 4×UINT + 2×SIZE_T + LUID.
        const DESCRIPTION_BYTES: usize = 128 * 2;
        const UINT_FIELDS_BYTES: usize = 4 * 4;
        const SIZE_T_FIELDS_BYTES: usize = 2 * std::mem::size_of::<usize>();
        const LUID_BYTES: usize = 8;
        let expected =
            DESCRIPTION_BYTES + UINT_FIELDS_BYTES + SIZE_T_FIELDS_BYTES + LUID_BYTES;
        assert_eq!(std::mem::size_of::<DxgiAdapterDesc>(), expected);
    }
}
