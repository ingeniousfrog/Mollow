use std::ffi::{OsStr, c_void};
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::ptr;

use mollow_core::ThermalInfo;

const COINIT_MULTITHREADED: u32 = 0x0000_0000;
const RPC_C_AUTHN_LEVEL_NONE: u32 = 0;
const RPC_C_IMP_LEVEL_IMPERSONATE: u32 = 3;
const EOAC_NONE: u32 = 0;
const WBEM_FLAG_FORWARD_ONLY: i32 = 0x0000_0030;
const ERROR_SUCCESS: i32 = 0;

#[repr(C)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

static CLSID_WbemLocator: Guid = Guid {
    data1: 0x4590f811,
    data2: 0x1d3a,
    data3: 0x11d0,
    data4: [0x89, 0x1f, 0x00, 0xaa, 0x00, 0x4b, 0x2e, 0x24],
};

static IID_IWbemLocator: Guid = Guid {
    data1: 0xdc12a687,
    data2: 0x737f,
    data3: 0x11cf,
    data4: [0x88, 0x4d, 0x00, 0xaa, 0x00, 0x4b, 0x2e, 0x24],
};

#[link(name = "ole32")]
unsafe extern "system" {
    fn CoInitializeEx(reserved: *mut c_void, coinit: u32) -> i32;
    fn CoUninitialize();
    fn CoCreateInstance(
        clsid: *const Guid,
        outer: *mut c_void,
        context: u32,
        iid: *const Guid,
        instance: *mut *mut c_void,
    ) -> i32;
    fn CoSetProxyBlanket(
        proxy: *mut c_void,
        authn: u32,
        authz: u32,
        server: *mut u16,
        authn_level: u32,
        imp_level: u32,
        auth_info: *mut c_void,
        capabilities: u32,
    ) -> i32;
}

type ReleaseFn = unsafe extern "system" fn(*mut c_void) -> u32;
type ConnectServerFn = unsafe extern "system" fn(
    *mut c_void,
    *const u16,
    *const u16,
    *const u16,
    *const u16,
    i32,
    *const u16,
    *mut c_void,
    *mut *mut c_void,
) -> i32;
type ExecQueryFn = unsafe extern "system" fn(
    *mut c_void,
    *const u16,
    *const u16,
    i32,
    *mut c_void,
    *mut *mut c_void,
) -> i32;
type NextFn = unsafe extern "system" fn(*mut c_void, i32, *mut *mut c_void, *mut u32) -> i32;
type GetFn =
    unsafe extern "system" fn(*mut c_void, *const u16, i32, *mut c_void, *mut u16, *mut u32) -> i32;

pub(crate) fn detect_thermal() -> io::Result<ThermalInfo> {
    // SAFETY: WMI initialization follows the documented COM contract.
    unsafe {
        let init = CoInitializeEx(ptr::null_mut(), COINIT_MULTITHREADED);
        if init != ERROR_SUCCESS && init != 1 {
            return Err(io::Error::from_raw_os_error(init));
        }

        let result = query_thermal_zone();
        CoUninitialize();
        result
    }
}

unsafe fn query_thermal_zone() -> io::Result<ThermalInfo> {
    let mut locator = ptr::null_mut();
    if CoCreateInstance(
        &raw const CLSID_WbemLocator,
        ptr::null_mut(),
        1,
        &raw const IID_IWbemLocator,
        &raw mut locator,
    ) != ERROR_SUCCESS
    {
        return Err(io::Error::last_os_error());
    }

    let result = query_from_locator(locator);
    release_com_object(locator);
    result
}

unsafe fn query_from_locator(locator: *mut c_void) -> io::Result<ThermalInfo> {
    let connect_server: ConnectServerFn = vtable_fn(locator, 3)?;
    let mut services = ptr::null_mut();
    let namespace = wide("ROOT\\WMI");
    if connect_server(
        locator,
        namespace.as_ptr(),
        ptr::null(),
        ptr::null(),
        ptr::null(),
        0,
        ptr::null(),
        ptr::null_mut(),
        &raw mut services,
    ) != ERROR_SUCCESS
    {
        return Err(io::Error::last_os_error());
    }

    let _ = CoSetProxyBlanket(
        services,
        RPC_C_AUTHN_LEVEL_NONE,
        RPC_C_IMP_LEVEL_IMPERSONATE,
        ptr::null_mut(),
        RPC_C_AUTHN_LEVEL_NONE,
        RPC_C_IMP_LEVEL_IMPERSONATE,
        ptr::null_mut(),
        EOAC_NONE,
    );

    let result = execute_thermal_query(services);
    release_com_object(services);
    result
}

unsafe fn execute_thermal_query(services: *mut c_void) -> io::Result<ThermalInfo> {
    let exec_query: ExecQueryFn = vtable_fn(services, 20)?;
    let mut enumerator = ptr::null_mut();
    let language = wide("WQL");
    let query = wide("SELECT CurrentTemperature FROM MSAcpi_ThermalZoneTemperature");
    if exec_query(
        services,
        language.as_ptr(),
        query.as_ptr(),
        WBEM_FLAG_FORWARD_ONLY,
        ptr::null_mut(),
        &raw mut enumerator,
    ) != ERROR_SUCCESS
    {
        return Err(io::Error::last_os_error());
    }

    let result = read_first_temperature(enumerator);
    release_com_object(enumerator);
    result
}

unsafe fn read_first_temperature(enumerator: *mut c_void) -> io::Result<ThermalInfo> {
    let next: NextFn = vtable_fn(enumerator, 4)?;
    let mut object = ptr::null_mut();
    let mut returned = 0_u32;
    if next(enumerator, 0xFFFF_FFFF, &raw mut object, &raw mut returned) != ERROR_SUCCESS
        || returned == 0
    {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no WMI thermal zones were returned",
        ));
    }

    let result = read_temperature_property(object);
    release_com_object(object);
    result
}

unsafe fn read_temperature_property(object: *mut c_void) -> io::Result<ThermalInfo> {
    let get: GetFn = vtable_fn(object, 4)?;
    let property = wide("CurrentTemperature");
    let mut variant = 0_u16;
    let mut flags = 0_u32;
    if get(
        object,
        property.as_ptr(),
        0,
        ptr::null_mut(),
        &raw mut variant,
        &raw mut flags,
    ) != ERROR_SUCCESS
    {
        return Err(io::Error::last_os_error());
    }

    let temperature = i64::from(variant);
    let milli_celsius = temperature
        .checked_sub(2732)
        .and_then(|value| value.checked_mul(100))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid WMI temperature"))?;

    Ok(ThermalInfo {
        state: if milli_celsius >= 90_000 {
            "critical"
        } else if milli_celsius >= 80_000 {
            "warning"
        } else {
            "normal"
        }
        .to_owned(),
        temperature_milli_celsius: Some(milli_celsius),
        sensor: Some("MSAcpi_ThermalZoneTemperature".to_owned()),
    })
}

unsafe fn vtable_fn<T>(object: *mut c_void, index: usize) -> io::Result<T> {
    if object.is_null() {
        return Err(io::Error::other("COM object pointer was null"));
    }
    let vtable = *(object.cast::<*const usize>());
    Ok(std::mem::transmute_copy(&*vtable.add(index)))
}

unsafe fn release_com_object(object: *mut c_void) {
    if object.is_null() {
        return;
    }
    let release: ReleaseFn = std::mem::transmute(*(*(object.cast::<*const usize>())).add(2));
    let _ = release(object);
}

fn wide(value: &str) -> Vec<u16> {
    OsStr::new(value).encode_wide().chain([0]).collect()
}
