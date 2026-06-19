use std::ffi::c_void;
use std::io;
use std::ptr;

use mollow_core::MediaInfo;

const ERROR_SUCCESS: i32 = 0;
const MF_VERSION: u32 = 0x0002_0070;
const MFSTARTUP_NOSOCKET: u32 = 0x0000_0001;
const MFT_ENUM_FLAG_HARDWARE: u32 = 0x0000_0004;
const MFT_ENUM_FLAG_SORTANDFILTER: u32 = 0x0000_0040;

#[repr(C)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

static MF_MEDIA_TYPE_VIDEO: Guid = Guid {
    data1: 0x7364_6976,
    data2: 0x0000,
    data3: 0x0010,
    data4: [0x80, 0x00, 0x00, 0xaa, 0x00, 0x38, 0x9b, 0x71],
};

static MFVIDEO_FORMAT_H264: Guid = Guid {
    data1: 0x3436_3248,
    data2: 0x0000,
    data3: 0x0010,
    data4: [0x80, 0x00, 0x00, 0xaa, 0x00, 0x38, 0x9b, 0x71],
};

static MFVIDEO_FORMAT_HEVC: Guid = Guid {
    data1: 0x4356_4548,
    data2: 0x0000,
    data3: 0x0010,
    data4: [0x80, 0x00, 0x00, 0xaa, 0x00, 0x38, 0x9b, 0x71],
};

static MFT_CATEGORY_VIDEO_DECODER: Guid = Guid {
    data1: 0x4d10_677c,
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
    fn CoTaskMemFree(ptr: *mut c_void);
}

pub(crate) fn detect_media() -> io::Result<MediaInfo> {
    // SAFETY: Media Foundation startup follows the documented COM contract.
    unsafe {
        if MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET) != ERROR_SUCCESS {
            return Err(io::Error::last_os_error());
        }

        let result = detect_with_media_foundation();
        let _ = MFShutdown();
        result
    }
}

unsafe fn detect_with_media_foundation() -> io::Result<MediaInfo> {
    // SAFETY: Media Foundation decoder enumeration uses valid COM pointers.
    unsafe {
        let mut decode_codecs = Vec::new();
        if hardware_decoder_exists(&MFVIDEO_FORMAT_H264)? {
            decode_codecs.push("h264".to_owned());
        }
        if hardware_decoder_exists(&MFVIDEO_FORMAT_HEVC)? {
            decode_codecs.push("hevc".to_owned());
        }

        if decode_codecs.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "no hardware video decoders were enumerated",
            ));
        }

        Ok(MediaInfo {
            backend: "Media Foundation".to_owned(),
            hardware_decode_codecs: decode_codecs,
            hardware_encode_codecs: Vec::new(),
            notes: vec!["hardware encode enumeration is not exposed by this probe".to_owned()],
        })
    }
}

unsafe fn hardware_decoder_exists(format: &Guid) -> io::Result<bool> {
    // SAFETY: MFTEnumEx is invoked with documented Media Foundation parameters.
    unsafe {
        let mut activate = ptr::null_mut();
        let mut count = 0_u32;
        let status = MFTEnumEx(
            &raw const MFT_CATEGORY_VIDEO_DECODER,
            MFT_ENUM_FLAG_HARDWARE | MFT_ENUM_FLAG_SORTANDFILTER,
            &raw const MF_MEDIA_TYPE_VIDEO,
            format,
            &raw mut activate,
            &raw mut count,
        );
        if status != ERROR_SUCCESS {
            return Err(io::Error::last_os_error());
        }
        if !activate.is_null() {
            CoTaskMemFree(activate.cast());
        }
        Ok(count > 0)
    }
}
