use std::io::{Read, Seek};
use lofty::probe::Probe;
use lofty::file::AudioFile;

pub struct TechnicalImageMetadata {
    pub width: Option<u32>,
    pub height: Option<u32>,
}

pub struct TechnicalAudioMetadata {
    pub bitrate_kbps: Option<u32>,
    pub duration_seconds: Option<u32>,
    pub sample_rate_hz: Option<u32>,
    pub channels: Option<u8>,
    pub codec: Option<String>,
}

pub struct TechnicalVideoMetadata {
    pub duration_seconds: Option<u32>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub frame_rate: Option<u32>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
}

pub fn extract_image_metadata<R: Read + Seek>(reader: R) -> TechnicalImageMetadata {
    let buffered = std::io::BufReader::new(reader);
    match image::ImageReader::new(buffered)
        .with_guessed_format()
        .ok()
        .and_then(|r| r.into_dimensions().ok())
    {
        Some((w, h)) => TechnicalImageMetadata {
            width: Some(w),
            height: Some(h),
        },
        None => TechnicalImageMetadata {
            width: None,
            height: None,
        },
    }
}

pub fn extract_audio_metadata<R: Read + Seek>(mut reader: R) -> TechnicalAudioMetadata {
    let probed = Probe::new(&mut reader).guess_file_type().ok();
    
    if let Some(p) = probed {
        let file_type = p.file_type();
        if let Ok(tagged_file) = p.read() {
            let properties = tagged_file.properties();
            return TechnicalAudioMetadata {
                bitrate_kbps: properties.audio_bitrate(),
                duration_seconds: Some(properties.duration().as_secs() as u32),
                sample_rate_hz: properties.sample_rate(),
                channels: properties.channels().map(|c| c as u8),
                codec: Some(format!("{:?}", file_type)),
            };
        }
    }

    TechnicalAudioMetadata {
        bitrate_kbps: None,
        duration_seconds: None,
        sample_rate_hz: None,
        channels: None,
        codec: None,
    }
}

pub fn extract_video_metadata<R: Read + Seek>(mut reader: R, size: u64, extension: &str) -> TechnicalVideoMetadata {
    if extension == "mp4" || extension == "mov" {
        if let Ok(mp4) = mp4::Mp4Reader::read_header(&mut reader, size) {
            let mut metadata = TechnicalVideoMetadata {
                duration_seconds: Some(mp4.duration().as_secs() as u32),
                width: None,
                height: None,
                frame_rate: None,
                video_codec: None,
                audio_codec: None,
            };

            for track in mp4.tracks().values() {
                match track.track_type() {
                    Ok(mp4::TrackType::Video) => {
                        metadata.width = Some(track.width() as u32);
                        metadata.height = Some(track.height() as u32);
                        metadata.video_codec = track.media_type().ok().map(|m| format!("{:?}", m));
                        metadata.frame_rate = Some(track.frame_rate().round() as u32);
                    }
                    Ok(mp4::TrackType::Audio) => {
                        metadata.audio_codec = track.media_type().ok().map(|m| format!("{:?}", m));
                    }
                    _ => {}
                }
            }
            return metadata;
        }
    }

    TechnicalVideoMetadata {
        duration_seconds: None,
        width: None,
        height: None,
        frame_rate: None,
        video_codec: None,
        audio_codec: None,
    }
}