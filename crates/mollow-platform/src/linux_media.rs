use std::fs;
use std::io;
use std::path::Path;

pub(crate) fn v4l2_codecs() -> io::Result<Vec<String>> {
    let devices = fs::read_dir("/sys/class/video4linux")
        .map_err(|error| io::Error::new(io::ErrorKind::NotFound, error))?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("video"))
        .count();
    if devices == 0 {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no V4L2 devices were found",
        ));
    }

    let mut codecs = Vec::new();
    for device in fs::read_dir("/sys/class/video4linux")
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
    {
        let name_path = device.path().join("name");
        let Ok(name) = fs::read_to_string(name_path) else {
            continue;
        };
        let name = name.trim().to_ascii_lowercase();
        if name.contains("h264") {
            codecs.push("h264".to_owned());
        }
        if name.contains("hevc") || name.contains("h265") {
            codecs.push("hevc".to_owned());
        }
        if name.contains("vp9") {
            codecs.push("vp9".to_owned());
        }
        if name.contains("av1") {
            codecs.push("av1".to_owned());
        }
    }

    if codecs.is_empty() {
        codecs.push("v4l2-device-present".to_owned());
    }
    codecs.sort();
    codecs.dedup();
    Ok(codecs)
}

pub(crate) fn vaapi_codecs() -> io::Result<Vec<String>> {
    let render_nodes = fs::read_dir("/dev/dri")
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("renderD"))
        .count();
    if render_nodes == 0 {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no DRM render node was found for VA-API",
        ));
    }

    let libva_paths = [
        "/usr/lib/x86_64-linux-gnu/libva.so.2",
        "/usr/lib64/libva.so.2",
        "/usr/lib/libva.so.2",
        "/usr/lib/aarch64-linux-gnu/libva.so.2",
    ];
    if !libva_paths.iter().any(|path| Path::new(path).exists()) {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "libva was not found on this system",
        ));
    }

    Ok(vec!["h264".to_owned(), "hevc".to_owned(), "vp9".to_owned()])
}
