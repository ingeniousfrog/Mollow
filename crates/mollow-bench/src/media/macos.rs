use std::ffi::c_void;
use std::ptr;

use mollow_core::BenchmarkProfile;

use mollow_core::BenchmarkSample;

use crate::BenchmarkError;
use crate::statistics::summarize;
use crate::workloads::{configuration, parameter, sample_from_elapsed};

const WORKLOAD_ID: &str = "media.videotoolbox-h264-encode";
const WORKLOAD_VERSION: u32 = 2;
const NO_ERR: i32 = 0;
const WIDTH: i32 = 640;
const HEIGHT: i32 = 352;
const TIMESCALE: i32 = 600;
const CM_TIME_FLAGS_VALID: u32 = 1;
const K_CV_PIXEL_FORMAT_TYPE_420V: u32 = 0x3432_3076; // '420v'
const K_CM_VIDEO_CODEC_TYPE_H264: u32 = 0x6176_6331; // 'avc1'

type CFAllocatorRef = *const c_void;
type CFTypeRef = *const c_void;
type CVPixelBufferRef = CFTypeRef;
type VTCompressionSessionRef = *mut c_void;
type OSStatus = i32;
type VTCompressionOutputCallback =
    unsafe extern "C-unwind" fn(*mut c_void, *mut c_void, OSStatus, u32, CFTypeRef);

#[repr(C)]
#[derive(Clone, Copy)]
struct CMTime {
    value: i64,
    timescale: i32,
    flags: u32,
    epoch: i64,
}

#[link(name = "CoreVideo", kind = "framework")]
unsafe extern "C" {
    fn CVPixelBufferCreate(
        allocator: CFAllocatorRef,
        width: usize,
        height: usize,
        pixel_format_type: u32,
        pixel_buffer_attributes: *const c_void,
        pixel_buffer_out: *mut CVPixelBufferRef,
    ) -> i32;
    fn CVPixelBufferLockBaseAddress(pixel_buffer: CVPixelBufferRef, lock_flags: u64) -> i32;
    fn CVPixelBufferUnlockBaseAddress(pixel_buffer: CVPixelBufferRef, lock_flags: u64) -> i32;
    fn CVPixelBufferGetBaseAddressOfPlane(
        pixel_buffer: CVPixelBufferRef,
        plane_index: usize,
    ) -> *mut c_void;
    fn CVPixelBufferGetBytesPerRowOfPlane(
        pixel_buffer: CVPixelBufferRef,
        plane_index: usize,
    ) -> usize;
    fn CVPixelBufferGetHeightOfPlane(pixel_buffer: CVPixelBufferRef, plane_index: usize) -> usize;
}

#[link(name = "VideoToolbox", kind = "framework")]
unsafe extern "C" {
    fn VTCompressionSessionCreate(
        allocator: CFAllocatorRef,
        width: i32,
        height: i32,
        codec_type: u32,
        encoder_specification: *const c_void,
        image_buffer_attributes: *const c_void,
        compressed_data_allocator: CFAllocatorRef,
        output_callback: VTCompressionOutputCallback,
        output_callback_ref_con: *mut c_void,
        compression_session_out: *mut VTCompressionSessionRef,
    ) -> OSStatus;
    fn VTCompressionSessionPrepareToEncodeFrames(session: VTCompressionSessionRef) -> OSStatus;
    fn VTCompressionSessionEncodeFrame(
        session: VTCompressionSessionRef,
        image_buffer: CVPixelBufferRef,
        presentation_time_stamp: CMTime,
        duration: CMTime,
        frame_properties: *const c_void,
        source_frame_refcon: *mut c_void,
        info_flags_out: *mut u32,
    ) -> OSStatus;
    fn VTCompressionSessionCompleteFrames(
        session: VTCompressionSessionRef,
        complete_until_presentation_time_stamp: CMTime,
    ) -> OSStatus;
    fn VTCompressionSessionInvalidate(session: VTCompressionSessionRef);
}

pub(crate) fn run(
    profile: BenchmarkProfile,
) -> Result<mollow_core::WorkloadResult, BenchmarkError> {
    let configuration = configuration(profile);
    let frames_per_sample = match profile {
        BenchmarkProfile::Quick => 8_u64,
        BenchmarkProfile::Standard => 16_u64,
    };
    let mut encoder = H264Encoder::new()?;

    for _ in 0..configuration.media_warmup_iterations {
        encoder.encode_frames(frames_per_sample)?;
    }

    let samples = (0..configuration.sample_count)
        .map(|_| {
            let started = std::time::Instant::now();
            let frames = encoder.encode_frames(frames_per_sample)?;
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
            parameter("backend", "VideoToolbox"),
            parameter("operation", "hardware-encode"),
            parameter("width", &WIDTH.to_string()),
            parameter("height", &HEIGHT.to_string()),
            parameter("frames_per_sample", &frames_per_sample.to_string()),
        ],
        samples,
        summary,
    })
}

struct H264Encoder {
    session: VTCompressionSessionRef,
    pixel_buffer: CVPixelBufferRef,
    next_frame_index: u64,
}

impl H264Encoder {
    fn new() -> Result<Self, BenchmarkError> {
        // SAFETY: CoreVideo and VideoToolbox calls follow documented ownership rules.
        unsafe {
            let mut pixel_buffer = ptr::null();
            let status = CVPixelBufferCreate(
                ptr::null(),
                usize::try_from(WIDTH)
                    .map_err(|error| BenchmarkError::new("media", error.to_string()))?,
                usize::try_from(HEIGHT)
                    .map_err(|error| BenchmarkError::new("media", error.to_string()))?,
                K_CV_PIXEL_FORMAT_TYPE_420V,
                ptr::null(),
                &raw mut pixel_buffer,
            );
            if status != NO_ERR || pixel_buffer.is_null() {
                return Err(BenchmarkError::new(
                    "media",
                    format!("CVPixelBufferCreate failed with status {status}"),
                ));
            }
            fill_pixel_buffer(pixel_buffer)?;

            let mut session = ptr::null_mut();
            let status = VTCompressionSessionCreate(
                ptr::null(),
                WIDTH,
                HEIGHT,
                K_CM_VIDEO_CODEC_TYPE_H264,
                ptr::null(),
                ptr::null(),
                ptr::null(),
                vt_compression_output_callback,
                ptr::null_mut(),
                &raw mut session,
            );
            if status != NO_ERR || session.is_null() {
                return Err(BenchmarkError::new(
                    "media",
                    format!("VTCompressionSessionCreate failed with status {status}"),
                ));
            }
            if VTCompressionSessionPrepareToEncodeFrames(session) != NO_ERR {
                VTCompressionSessionInvalidate(session);
                return Err(BenchmarkError::new(
                    "media",
                    "VTCompressionSessionPrepareToEncodeFrames failed",
                ));
            }
            Ok(Self {
                session,
                pixel_buffer,
                next_frame_index: 0,
            })
        }
    }

    fn encode_frames(&mut self, frames: u64) -> Result<u64, BenchmarkError> {
        let duration = CMTime {
            value: 1,
            timescale: TIMESCALE,
            flags: CM_TIME_FLAGS_VALID,
            epoch: 0,
        };
        for _ in 0..frames {
            let presentation = CMTime {
                value: i64::try_from(self.next_frame_index)
                    .map_err(|error| BenchmarkError::new("media", error.to_string()))?,
                timescale: TIMESCALE,
                flags: CM_TIME_FLAGS_VALID,
                epoch: 0,
            };
            self.next_frame_index += 1;
            // SAFETY: Compression session and pixel buffer were created by VideoToolbox.
            unsafe {
                let mut info_flags = 0_u32;
                let status = VTCompressionSessionEncodeFrame(
                    self.session,
                    self.pixel_buffer,
                    presentation,
                    duration,
                    ptr::null(),
                    ptr::null_mut(),
                    &raw mut info_flags,
                );
                if status != NO_ERR {
                    return Err(BenchmarkError::new(
                        "media",
                        format!("VTCompressionSessionEncodeFrame failed with status {status}"),
                    ));
                }
            }
        }
        // SAFETY: Completing frames drains the encoder output queue.
        unsafe {
            let complete_time = CMTime {
                value: i64::MAX,
                timescale: TIMESCALE,
                flags: CM_TIME_FLAGS_VALID,
                epoch: 0,
            };
            if VTCompressionSessionCompleteFrames(self.session, complete_time) != NO_ERR {
                return Err(BenchmarkError::new(
                    "media",
                    "VTCompressionSessionCompleteFrames failed",
                ));
            }
        }
        Ok(frames)
    }
}

impl Drop for H264Encoder {
    fn drop(&mut self) {
        // SAFETY: Invalidating the session releases native encoder resources.
        unsafe {
            if !self.session.is_null() {
                VTCompressionSessionInvalidate(self.session);
            }
        }
    }
}

unsafe extern "C-unwind" fn vt_compression_output_callback(
    _output_callback_ref_con: *mut c_void,
    _source_frame_ref_con: *mut c_void,
    _status: OSStatus,
    _info_flags: u32,
    _sample_buffer: CFTypeRef,
) {
}

unsafe fn fill_pixel_buffer(pixel_buffer: CVPixelBufferRef) -> Result<(), BenchmarkError> {
    // SAFETY: Pixel buffer memory is locked and written through CoreVideo plane APIs.
    unsafe {
        if CVPixelBufferLockBaseAddress(pixel_buffer, 0) != NO_ERR {
            return Err(BenchmarkError::new(
                "media",
                "CVPixelBufferLockBaseAddress failed",
            ));
        }
        let result = (|| {
            let y_plane = CVPixelBufferGetBaseAddressOfPlane(pixel_buffer, 0);
            let y_stride = CVPixelBufferGetBytesPerRowOfPlane(pixel_buffer, 0);
            let y_height = CVPixelBufferGetHeightOfPlane(pixel_buffer, 0);
            if y_plane.is_null() || y_stride == 0 || y_height == 0 {
                return Err(BenchmarkError::new("media", "Y plane was unavailable"));
            }
            for row in 0..y_height {
                let offset = row * y_stride;
                let row_ptr = y_plane.add(offset).cast::<u8>();
                for column in 0..usize::try_from(WIDTH).unwrap_or(0) {
                    *row_ptr.add(column) = u8::try_from((row + column) & 0xff)
                        .map_err(|error| BenchmarkError::new("media", error.to_string()))?;
                }
            }
            let uv_plane = CVPixelBufferGetBaseAddressOfPlane(pixel_buffer, 1);
            let uv_stride = CVPixelBufferGetBytesPerRowOfPlane(pixel_buffer, 1);
            let uv_height = CVPixelBufferGetHeightOfPlane(pixel_buffer, 1);
            if uv_plane.is_null() || uv_stride == 0 || uv_height == 0 {
                return Err(BenchmarkError::new("media", "UV plane was unavailable"));
            }
            for row in 0..uv_height {
                let offset = row * uv_stride;
                let row_ptr = uv_plane.add(offset).cast::<u8>();
                for column in 0..uv_stride {
                    *row_ptr.add(column) = 0x80;
                }
            }
            Ok(())
        })();
        let _ = CVPixelBufferUnlockBaseAddress(pixel_buffer, 0);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn videotoolbox_encoder_encodes_frames() {
        let mut encoder = H264Encoder::new().expect("encoder should initialize");
        encoder
            .encode_frames(4)
            .expect("encoder should produce compressed frames");
    }
}
