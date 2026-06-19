use std::ffi::c_void;
use std::ptr;

use mollow_core::BenchmarkProfile;

use mollow_core::BenchmarkSample;

use crate::BenchmarkError;
use crate::media::{H264_FIXTURE, split_annex_b_nals};
use crate::statistics::summarize;
use crate::workloads::{BenchmarkSample, configuration, parameter, sample_from_elapsed};

const WORKLOAD_ID: &str = "media.media-foundation-h264-decode";
const WORKLOAD_VERSION: u32 = 2;
const ERROR_SUCCESS: i32 = 0;
const MF_VERSION: u32 = 0x0002_0070;
const MFSTARTUP_NOSOCKET: u32 = 0x0000_0001;
const MFT_ENUM_FLAG_HARDWARE: u32 = 0x0000_0004;
const MFT_ENUM_FLAG_SORTANDFILTER: u32 = 0x0000_0040;
const MF_E_TRANSFORM_NEED_MORE_INPUT: i32 = 0xC00D6D72_u32 as i32;

#[repr(C)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

static MF_MEDIA_TYPE_VIDEO: Guid = Guid {
    data1: 0x73646976,
    data2: 0x0000,
    data3: 0x0010,
    data4: [0x80, 0x00, 0x00, 0xaa, 0x00, 0x38, 0x9b, 0x71],
};

static MFVideoFormat_H264: Guid = Guid {
    data1: 0x34363248,
    data2: 0x0000,
    data3: 0x0010,
    data4: [0x80, 0x00, 0x00, 0xaa, 0x00, 0x38, 0x9b, 0x71],
};

static MFT_CATEGORY_VIDEO_DECODER: Guid = Guid {
    data1: 0x4d10677c,
    data2: 0x40ef,
    data3: 0x11d3,
    data4: [0x97, 0x3c, 0x00, 0xa0, 0xc9, 0xa0, 0x6a, 0x65],
};

#[link(name = "mfplat")]
unsafe extern "system" {
    fn MFStartup(version: u32, flags: u32) -> i32;
    fn MFShutdown() -> i32;
    fn MFTEnumEx(
        guid_category: *const Guid,
        flags: u32,
        input_type: *const Guid,
        output_type: *const Guid,
        activate: *mut *mut *mut c_void,
        count: *mut u32,
    ) -> i32;
    fn MFCreateSample(sample: *mut *mut c_void) -> i32;
    fn MFCreateMemoryBuffer(max_length: u32, buffer: *mut *mut c_void) -> i32;
    fn MFCreateMediaType(media_type: *mut *mut c_void) -> i32;
    fn CoTaskMemFree(ptr: *mut c_void);
}

type ReleaseFn = unsafe extern "system" fn(*mut c_void) -> u32;
type ActivateObjectFn =
    unsafe extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void) -> i32;
type SetInputTypeFn = unsafe extern "system" fn(*mut c_void, u32, *mut c_void, u32) -> i32;
type ProcessInputFn = unsafe extern "system" fn(*mut c_void, u32, *mut c_void, u32) -> i32;
type ProcessOutputFn =
    unsafe extern "system" fn(*mut c_void, u32, u32, *mut c_void, *mut u32) -> i32;
type GetBufferLengthFn = unsafe extern "system" fn(*mut c_void, *mut u32) -> i32;
type LockFn = unsafe extern "system" fn(*mut c_void, *mut *mut u8, *mut u32, *mut u32) -> i32;
type UnlockFn = unsafe extern "system" fn(*mut c_void) -> i32;
type SetUINT32Fn = unsafe extern "system" fn(*mut c_void, u32, u32) -> i32;
type AddBufferFn = unsafe extern "system" fn(*mut c_void, *mut c_void) -> i32;

static IID_IMFTransform: Guid = Guid {
    data1: 0xbf94c121,
    data2: 0x5b05,
    data3: 0x4e6f,
    data4: [0x80, 0x00, 0x00, 0xaa, 0x00, 0x38, 0x9b, 0x71],
};

pub(crate) fn run(
    profile: BenchmarkProfile,
) -> Result<mollow_core::WorkloadResult, BenchmarkError> {
    let configuration = configuration(profile);
    let frames_per_sample = match profile {
        BenchmarkProfile::Quick => 8_u64,
        BenchmarkProfile::Standard => 16_u64,
    };

    // SAFETY: Media Foundation startup follows the documented COM contract.
    unsafe {
        if MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET) != ERROR_SUCCESS {
            return Err(BenchmarkError::new("media", "MFStartup failed"));
        }
    }

    let result = run_with_media_foundation(profile, frames_per_sample, configuration);
    unsafe {
        let _ = MFShutdown();
    }
    result
}

fn run_with_media_foundation(
    profile: BenchmarkProfile,
    frames_per_sample: u64,
    configuration: crate::workloads::Configuration,
) -> Result<mollow_core::WorkloadResult, BenchmarkError> {
    let decoder = H264Decoder::open()?;
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
    let _ = profile;
    Ok(mollow_core::WorkloadResult {
        workload_id: WORKLOAD_ID.to_owned(),
        workload_version: WORKLOAD_VERSION,
        measurement: "frames_per_second".to_owned(),
        warmup_iterations: configuration.media_warmup_iterations,
        parameters: vec![
            parameter("codec", "h264"),
            parameter("backend", "Media Foundation"),
            parameter("fixture", "minimal-baseline.h264"),
            parameter("frames_per_sample", &frames_per_sample.to_string()),
        ],
        samples,
        summary,
    })
}

struct H264Decoder {
    transform: *mut c_void,
    bitstream: Vec<u8>,
}

impl H264Decoder {
    fn open() -> Result<Self, BenchmarkError> {
        let nals = split_annex_b_nals(H264_FIXTURE);
        let bitstream = nals
            .into_iter()
            .flat_map(|nal| {
                let mut prefixed = vec![0_u8, 0, 0, 1];
                prefixed.extend_from_slice(nal);
                prefixed
            })
            .collect::<Vec<_>>();
        // SAFETY: MFT activation follows the documented Media Foundation contract.
        unsafe {
            let mut activate = ptr::null_mut();
            let mut count = 0_u32;
            if MFTEnumEx(
                &raw const MFT_CATEGORY_VIDEO_DECODER,
                MFT_ENUM_FLAG_HARDWARE | MFT_ENUM_FLAG_SORTANDFILTER,
                &raw const MF_MEDIA_TYPE_VIDEO,
                &raw const MFVideoFormat_H264,
                &raw mut activate,
                &raw mut count,
            ) != ERROR_SUCCESS
                || count == 0
            {
                return Err(BenchmarkError::new(
                    "media",
                    "no hardware H.264 decoder MFT was found",
                ));
            }
            let transform = activate_object(activate)?;
            CoTaskMemFree(activate.cast());
            configure_input_type(transform)?;
            Ok(Self {
                transform,
                bitstream,
            })
        }
    }

    fn decode_frames(&self, frames: u64) -> Result<u64, BenchmarkError> {
        for _ in 0..frames {
            // SAFETY: IMFTransform methods are invoked through valid COM pointers.
            unsafe {
                let sample = create_sample(&self.bitstream)?;
                let process_input: ProcessInputFn = vtable_fn(self.transform, 19)?;
                let status = process_input(self.transform, 0, sample, 0);
                release_com_object(sample);
                if status != ERROR_SUCCESS && status != MF_E_TRANSFORM_NEED_MORE_INPUT {
                    return Err(BenchmarkError::new(
                        "media",
                        format!("IMFTransform::ProcessInput failed with status {status}"),
                    ));
                }
                let _status = process_output(self.transform)?;
            }
        }
        Ok(frames)
    }
}

impl Drop for H264Decoder {
    fn drop(&mut self) {
        // SAFETY: COM release decrements the transform reference count.
        unsafe {
            release_com_object(self.transform);
        }
    }
}

unsafe fn activate_object(activate: *mut *mut c_void) -> Result<*mut c_void, BenchmarkError> {
    let activate_object: ActivateObjectFn = vtable_fn(*activate, 3)?;
    let mut transform = ptr::null_mut();
    if activate_object(*activate, &raw const IID_IMFTransform, &raw mut transform) != ERROR_SUCCESS
    {
        return Err(BenchmarkError::new(
            "media",
            "failed to activate decoder MFT",
        ));
    }
    Ok(transform)
}

unsafe fn configure_input_type(transform: *mut c_void) -> Result<(), BenchmarkError> {
    let mut media_type = ptr::null_mut();
    if MFCreateMediaType(&raw mut media_type) != ERROR_SUCCESS {
        return Err(BenchmarkError::new("media", "MFCreateMediaType failed"));
    }
    let set_uint32: SetUINT32Fn = vtable_fn(media_type, 7)?;
    let _ = set_uint32(media_type, 0, 0x34363248);
    let set_input_type: SetInputTypeFn = vtable_fn(transform, 13)?;
    let status = set_input_type(transform, 0, media_type, 0);
    release_com_object(media_type);
    if status != ERROR_SUCCESS {
        return Err(BenchmarkError::new(
            "media",
            format!("IMFTransform::SetInputType failed with status {status}"),
        ));
    }
    Ok(())
}

unsafe fn create_sample(bitstream: &[u8]) -> Result<*mut c_void, BenchmarkError> {
    let mut sample = ptr::null_mut();
    if MFCreateSample(&raw mut sample) != ERROR_SUCCESS {
        return Err(BenchmarkError::new("media", "MFCreateSample failed"));
    }
    let mut buffer = ptr::null_mut();
    let max_length = u32::try_from(bitstream.len())
        .map_err(|error| BenchmarkError::new("media", error.to_string()))?;
    if MFCreateMemoryBuffer(max_length, &raw mut buffer) != ERROR_SUCCESS {
        release_com_object(sample);
        return Err(BenchmarkError::new("media", "MFCreateMemoryBuffer failed"));
    }
    let lock: LockFn = vtable_fn(buffer, 4)?;
    let unlock: UnlockFn = vtable_fn(buffer, 5)?;
    let mut data = ptr::null_mut();
    let mut max = 0_u32;
    let mut current = 0_u32;
    if lock(buffer, &raw mut data, &raw mut max, &raw mut current) != ERROR_SUCCESS {
        release_com_object(buffer);
        release_com_object(sample);
        return Err(BenchmarkError::new("media", "IMFMediaBuffer::Lock failed"));
    }
    ptr::copy_nonoverlapping(bitstream.as_ptr(), data, bitstream.len());
    let _ = unlock(buffer);
    let add_buffer: AddBufferFn = vtable_fn(sample, 4)?;
    if add_buffer(sample, buffer) != ERROR_SUCCESS {
        release_com_object(buffer);
        release_com_object(sample);
        return Err(BenchmarkError::new("media", "IMFSample::AddBuffer failed"));
    }
    release_com_object(buffer);
    Ok(sample)
}

unsafe fn process_output(transform: *mut c_void) -> Result<(), BenchmarkError> {
    let process_output: ProcessOutputFn = vtable_fn(transform, 20)?;
    let mut status = 0_u32;
    let result = process_output(transform, 0, 1, ptr::null_mut(), &raw mut status);
    if result != ERROR_SUCCESS {
        return Err(BenchmarkError::new(
            "media",
            format!("IMFTransform::ProcessOutput failed with status {result}"),
        ));
    }
    Ok(())
}

unsafe fn vtable_fn<T>(object: *mut c_void, index: usize) -> Result<T, BenchmarkError> {
    if object.is_null() {
        return Err(BenchmarkError::new("media", "COM object pointer was null"));
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
