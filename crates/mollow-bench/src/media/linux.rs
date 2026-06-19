use std::ffi::{CString, c_char, c_void};
use std::fs;
use std::path::Path;
use std::ptr;

use mollow_core::BenchmarkProfile;

use mollow_core::BenchmarkSample;

use crate::BenchmarkError;
use crate::media::{H264_FIXTURE, split_annex_b_nals};
use crate::statistics::summarize;
use crate::workloads::{configuration, parameter, sample_from_elapsed};

const WORKLOAD_ID: &str = "media.vaapi-h264-decode";
const WORKLOAD_VERSION: u32 = 2;
const VA_STATUS_SUCCESS: i32 = 0;
const VA_PROFILE_H264_MAIN: i32 = 5;
const VA_ENTRYPOINT_VLD: i32 = 1;
const VA_RT_FORMAT_YUV420: u32 = 0x0000_0001;
const VA_FOURCC_NV12: u32 = 0x3231564e;
const VA_SLICE_DATA_BUFFER_TYPE: u32 = 0x0000_0002;

type VADisplay = *mut c_void;
type VAStatus = i32;

#[repr(C)]
struct VASliceParameterBuffer {
    slice_data_size: u32,
    slice_data_offset: u32,
    slice_data_flag: u32,
    slice_data_bit_offset: u16,
    first_mb_in_slice: u16,
    slice_type: u8,
    direct_spatial_mv_pred_flag: u8,
    num_ref_idx_l0_active_minus1: u8,
    num_ref_idx_l1_active_minus1: u8,
    cabac_init_idc: u8,
    slice_qp_delta: i8,
    luma_log2_weight_denom: u8,
    chroma_log2_weight_denom: u8,
    luma_weight_l0_flag: u8,
    chroma_weight_l0_flag: u8,
    luma_weight_l1_flag: u8,
    chroma_weight_l1_flag: u8,
    num_ref_idx_active_override_flag: u8,
    num_ref_idx_l0_active_minus1_override: u8,
    num_ref_idx_l1_active_minus1_override: u8,
}

pub(crate) fn run(
    profile: BenchmarkProfile,
) -> Result<mollow_core::WorkloadResult, BenchmarkError> {
    let configuration = configuration(profile);
    let frames_per_sample = match profile {
        BenchmarkProfile::Quick => 8_u64,
        BenchmarkProfile::Standard => 16_u64,
    };
    let decoder = VaapiDecoder::open()?;

    for _ in 0..configuration.media_warmup_iterations {
        decoder.decode_frames(frames_per_sample)?;
    }

    let samples = (0..configuration.sample_count)
        .map(|_| {
            let started = std::time::Instant::now();
            let frames = decoder.decode_frames(frames_per_sample)?;
            sample_from_elapsed(frames, started.elapsed().as_nanos(), "media")
        })
        .collect::<Result<Vec<BenchmarkSample>, _>>()?;

    let summary = summarize(&samples)?;
    Ok(mollow_core::WorkloadResult {
        workload_id: WORKLOAD_ID.to_owned(),
        workload_version: WORKLOAD_VERSION,
        measurement: "frames_per_second".to_owned(),
        warmup_iterations: configuration.media_warmup_iterations,
        parameters: vec![
            parameter("codec", "h264"),
            parameter("backend", "VA-API"),
            parameter("fixture", "minimal-baseline.h264"),
            parameter("frames_per_sample", &frames_per_sample.to_string()),
        ],
        samples,
        summary,
    })
}

struct VaapiDecoder {
    _library: *mut c_void,
    display: VADisplay,
    context: u32,
    surface: u32,
    bitstream: Vec<u8>,
    va_begin_picture: unsafe extern "C" fn(VADisplay, u32, u32) -> VAStatus,
    va_create_buffer:
        unsafe extern "C" fn(VADisplay, u32, u32, u32, u32, *mut c_void, *mut u32) -> VAStatus,
    va_destroy_buffer: unsafe extern "C" fn(VADisplay, u32) -> VAStatus,
    va_end_picture: unsafe extern "C" fn(VADisplay, u32) -> VAStatus,
    va_render_picture: unsafe extern "C" fn(VADisplay, u32, *const u32, i32) -> VAStatus,
    va_sync_surface: unsafe extern "C" fn(VADisplay, u32) -> VAStatus,
}

impl VaapiDecoder {
    fn open() -> Result<Self, BenchmarkError> {
        let render_node = find_render_node()?;
        let library = load_libva()?;
        // SAFETY: libva symbols are resolved from a successfully loaded library.
        unsafe {
            let va_get_display_drm: unsafe extern "C" fn(*const c_char) -> VADisplay =
                symbol(library, "vaGetDisplayDRM")?;
            let va_initialize: unsafe extern "C" fn(VADisplay, *mut i32, *mut i32) -> VAStatus =
                symbol(library, "vaInitialize")?;
            let va_create_config: unsafe extern "C" fn(
                VADisplay,
                i32,
                i32,
                *const i32,
                i32,
                *mut u32,
            ) -> VAStatus = symbol(library, "vaCreateConfig")?;
            let va_create_context: unsafe extern "C" fn(
                VADisplay,
                u32,
                i32,
                i32,
                i32,
                i32,
                *const u32,
                i32,
                *mut u32,
            ) -> VAStatus = symbol(library, "vaCreateContext")?;
            let va_create_surfaces: unsafe extern "C" fn(
                VADisplay,
                u32,
                u32,
                u32,
                i32,
                *mut u32,
                i32,
            ) -> VAStatus = symbol(library, "vaCreateSurfaces")?;
            let va_destroy_config: unsafe extern "C" fn(VADisplay, u32) -> VAStatus =
                symbol(library, "vaDestroyConfig")?;
            let va_destroy_context: unsafe extern "C" fn(VADisplay, u32) -> VAStatus =
                symbol(library, "vaDestroyContext")?;
            let va_destroy_surfaces: unsafe extern "C" fn(VADisplay, *const u32, i32) -> VAStatus =
                symbol(library, "vaDestroySurfaces")?;
            let va_terminate: unsafe extern "C" fn(VADisplay) -> VAStatus =
                symbol(library, "vaTerminate")?;

            let node = CString::new(render_node).map_err(|error| {
                BenchmarkError::new("media", format!("render node path was invalid: {error}"))
            })?;
            let display = va_get_display_drm(node.as_ptr());
            if display.is_null() {
                return Err(BenchmarkError::new(
                    "media",
                    "vaGetDisplayDRM returned null",
                ));
            }
            let mut major = 0;
            let mut minor = 0;
            if va_initialize(display, &raw mut major, &raw mut minor) != VA_STATUS_SUCCESS {
                return Err(BenchmarkError::new("media", "vaInitialize failed"));
            }

            let mut config = 0_u32;
            if va_create_config(
                display,
                VA_PROFILE_H264_MAIN,
                VA_ENTRYPOINT_VLD,
                ptr::null(),
                0,
                &raw mut config,
            ) != VA_STATUS_SUCCESS
            {
                let _ = va_terminate(display);
                return Err(BenchmarkError::new(
                    "media",
                    "vaCreateConfig failed for H.264 decode",
                ));
            }

            let mut surface = 0_u32;
            if va_create_surfaces(display, VA_RT_FORMAT_YUV420, 16, 16, 1, &raw mut surface, 1)
                != VA_STATUS_SUCCESS
            {
                let _ = va_destroy_config(display, config);
                let _ = va_terminate(display);
                return Err(BenchmarkError::new("media", "vaCreateSurfaces failed"));
            }

            let mut context = 0_u32;
            if va_create_context(
                display,
                config,
                16,
                16,
                VA_RT_FORMAT_YUV420,
                &surface,
                1,
                &raw mut context,
            ) != VA_STATUS_SUCCESS
            {
                let _ = va_destroy_surfaces(display, &surface, 1);
                let _ = va_destroy_config(display, config);
                let _ = va_terminate(display);
                return Err(BenchmarkError::new("media", "vaCreateContext failed"));
            }

            let nals = split_annex_b_nals(H264_FIXTURE);
            let bitstream = nals
                .into_iter()
                .flat_map(|nal| {
                    let mut prefixed = vec![0_u8, 0, 0, 1];
                    prefixed.extend_from_slice(nal);
                    prefixed
                })
                .collect::<Vec<_>>();

            Ok(Self {
                _library: library,
                display,
                context,
                surface,
                bitstream,
                va_begin_picture: symbol(library, "vaBeginPicture")?,
                va_create_buffer: symbol(library, "vaCreateBuffer")?,
                va_destroy_buffer: symbol(library, "vaDestroyBuffer")?,
                va_end_picture: symbol(library, "vaEndPicture")?,
                va_render_picture: symbol(library, "vaRenderPicture")?,
                va_sync_surface: symbol(library, "vaSyncSurface")?,
            })
        }
    }

    fn decode_frames(&self, frames: u64) -> Result<u64, BenchmarkError> {
        for _ in 0..frames {
            // SAFETY: VA-API calls use handles created during decoder initialization.
            unsafe {
                if (self.va_begin_picture)(self.display, self.context, self.surface)
                    != VA_STATUS_SUCCESS
                {
                    return Err(BenchmarkError::new("media", "vaBeginPicture failed"));
                }

                let mut slice_buffer = 0_u32;
                let slice_params = VASliceParameterBuffer {
                    slice_data_size: u32::try_from(self.bitstream.len())
                        .map_err(|error| BenchmarkError::new("media", error.to_string()))?,
                    slice_data_offset: 0,
                    slice_data_flag: 0,
                    slice_data_bit_offset: 0,
                    first_mb_in_slice: 0,
                    slice_type: 7,
                    direct_spatial_mv_pred_flag: 0,
                    num_ref_idx_l0_active_minus1: 0,
                    num_ref_idx_l1_active_minus1: 0,
                    cabac_init_idc: 0,
                    slice_qp_delta: 0,
                    luma_log2_weight_denom: 0,
                    chroma_log2_weight_denom: 0,
                    luma_weight_l0_flag: 0,
                    chroma_weight_l0_flag: 0,
                    luma_weight_l1_flag: 0,
                    chroma_weight_l1_flag: 0,
                    num_ref_idx_active_override_flag: 0,
                    num_ref_idx_l0_active_minus1_override: 0,
                    num_ref_idx_l1_active_minus1_override: 0,
                };
                if (self.va_create_buffer)(
                    self.display,
                    self.context,
                    VA_SLICE_DATA_BUFFER_TYPE,
                    std::mem::size_of::<VASliceParameterBuffer>() as u32,
                    1,
                    &raw const slice_params as *mut c_void,
                    &raw mut slice_buffer,
                ) != VA_STATUS_SUCCESS
                {
                    return Err(BenchmarkError::new("media", "vaCreateBuffer failed"));
                }

                let mut data_buffer = 0_u32;
                if (self.va_create_buffer)(
                    self.display,
                    self.context,
                    VA_SLICE_DATA_BUFFER_TYPE,
                    u32::try_from(self.bitstream.len())
                        .map_err(|error| BenchmarkError::new("media", error.to_string()))?,
                    1,
                    self.bitstream.as_ptr().cast_mut().cast(),
                    &raw mut data_buffer,
                ) != VA_STATUS_SUCCESS
                {
                    let _ = (self.va_destroy_buffer)(self.display, slice_buffer);
                    return Err(BenchmarkError::new(
                        "media",
                        "vaCreateBuffer failed for bitstream",
                    ));
                }

                let buffers = [slice_buffer, data_buffer];
                let status =
                    (self.va_render_picture)(self.display, self.context, buffers.as_ptr(), 2);
                let _ = (self.va_destroy_buffer)(self.display, slice_buffer);
                let _ = (self.va_destroy_buffer)(self.display, data_buffer);
                if status != VA_STATUS_SUCCESS {
                    return Err(BenchmarkError::new("media", "vaRenderPicture failed"));
                }
                if (self.va_end_picture)(self.display, self.context) != VA_STATUS_SUCCESS {
                    return Err(BenchmarkError::new("media", "vaEndPicture failed"));
                }
                if (self.va_sync_surface)(self.display, self.surface) != VA_STATUS_SUCCESS {
                    return Err(BenchmarkError::new("media", "vaSyncSurface failed"));
                }
            }
        }
        Ok(frames)
    }
}

impl Drop for VaapiDecoder {
    fn drop(&mut self) {
        // SAFETY: Tear down VA-API resources loaded through libva.
        unsafe {
            let va_destroy_context: Option<unsafe extern "C" fn(VADisplay, u32) -> VAStatus> =
                symbol(self._library, "vaDestroyContext").ok();
            let va_destroy_surfaces: Option<
                unsafe extern "C" fn(VADisplay, *const u32, i32) -> VAStatus,
            > = symbol(self._library, "vaDestroySurfaces").ok();
            let va_terminate: Option<unsafe extern "C" fn(VADisplay) -> VAStatus> =
                symbol(self._library, "vaTerminate").ok();
            if let Some(va_destroy_context) = va_destroy_context {
                let _ = va_destroy_context(self.display, self.context);
            }
            if let Some(va_destroy_surfaces) = va_destroy_surfaces {
                let _ = va_destroy_surfaces(self.display, &self.surface, 1);
            }
            if let Some(va_terminate) = va_terminate {
                let _ = va_terminate(self.display);
            }
            libc::dlclose(self._library);
        }
    }
}

fn find_render_node() -> Result<String, BenchmarkError> {
    let entries = fs::read_dir("/dev/dri").map_err(|error| {
        BenchmarkError::new("media", format!("could not read /dev/dri: {error}"))
    })?;
    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("renderD") {
            return Ok(format!("/dev/dri/{name}"));
        }
    }
    Err(BenchmarkError::new(
        "media",
        "no DRM render node was found for VA-API",
    ))
}

fn load_libva() -> Result<*mut c_void, BenchmarkError> {
    for path in [
        "/usr/lib/x86_64-linux-gnu/libva.so.2",
        "/usr/lib64/libva.so.2",
        "/usr/lib/libva.so.2",
        "/usr/lib/aarch64-linux-gnu/libva.so.2",
    ] {
        if !Path::new(path).exists() {
            continue;
        }
        let path = CString::new(path).map_err(|error| {
            BenchmarkError::new("media", format!("libva path was invalid: {error}"))
        })?;
        // SAFETY: libva is loaded with RTLD_NOW for symbol resolution.
        let library = unsafe { libc::dlopen(path.as_ptr(), libc::RTLD_NOW) };
        if library.is_null() {
            continue;
        }
        return Ok(library);
    }
    Err(BenchmarkError::new(
        "media",
        "libva was not found on this system",
    ))
}

unsafe fn symbol<T>(library: *mut c_void, name: &str) -> Result<T, BenchmarkError> {
    let name = CString::new(name)
        .map_err(|error| BenchmarkError::new("media", format!("symbol name invalid: {error}")))?;
    let symbol = libc::dlsym(library, name.as_ptr());
    if symbol.is_null() {
        return Err(BenchmarkError::new(
            "media",
            format!("missing libva symbol: {}", name.to_string_lossy()),
        ));
    }
    Ok(std::mem::transmute_copy(&symbol))
}

#[allow(dead_code)]
const _: () = {
    assert!(size_of::<VASliceParameterBuffer>() >= 24);
    assert_eq!(VA_FOURCC_NV12, 0x3231_564e);
};
