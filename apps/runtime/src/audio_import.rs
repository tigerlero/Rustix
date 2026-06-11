use std::path::Path;

use rustix_audio::{AudioError, SoundInstance, StreamDecoder};

/// Options controlling how an audio file is imported.
pub struct AudioImportOptions {
    /// Use streaming playback for long files (music). Short SFX should be `false`.
    pub streaming: bool,
    pub volume: f32,
    pub looping: bool,
    pub spatial_blend: f32,
}

impl Default for AudioImportOptions {
    fn default() -> Self {
        Self {
            streaming: false,
            volume: 1.0,
            looping: false,
            spatial_blend: 0.0,
        }
    }
}

/// Result of importing an audio file.
pub struct ImportedAudio {
    pub name: String,
    pub path: String,
    pub sample_rate: u32,
    pub channels: u16,
    /// Duration in seconds (estimated from frame count for streaming; actual for in-memory).
    pub duration_seconds: f32,
    /// Whether this asset is intended for streaming playback.
    pub streaming: bool,
    pub volume: f32,
    pub looping: bool,
    /// Fully-decoded interleaved f32 samples (empty when `streaming == true`).
    pub decoded: Vec<f32>,
}

/// Import an audio file (WAV, OGG, MP3, FLAC, AAC).
///
/// * `path` — filesystem path to the audio file.
/// * `name` — human-readable name used as the registry key.
/// * `options` — import options (streaming, volume, looping, spatial blend).
pub fn import_audio(
    path: &Path,
    name: &str,
    options: &AudioImportOptions,
) -> Result<ImportedAudio, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "wav" | "ogg" | "mp3" | "flac" | "aac" | "m4a" => {}
        _ => {
            return Err(format!("unsupported audio format: {ext}"));
        }
    }

    if options.streaming {
        // For streaming music, probe metadata without fully decoding.
        let (sample_rate, channels, duration_seconds) =
            probe_audio_metadata(path).map_err(|e| e.to_string())?;

        Ok(ImportedAudio {
            name: name.to_string(),
            path: path.to_string_lossy().to_string(),
            sample_rate,
            channels,
            duration_seconds,
            streaming: true,
            volume: options.volume,
            looping: options.looping,
            decoded: Vec::new(),
        })
    } else {
        // For SFX, fully decode into memory.
        let (decoded, sample_rate, channels) =
            rustix_audio::decoder::decode_audio(path).map_err(|e| e.to_string())?;

        let duration_seconds = decoded.len() as f32 / (sample_rate as f32 * channels as f32);

        Ok(ImportedAudio {
            name: name.to_string(),
            path: path.to_string_lossy().to_string(),
            sample_rate,
            channels,
            duration_seconds,
            streaming: false,
            volume: options.volume,
            looping: options.looping,
            decoded,
        })
    }
}

/// Probe metadata without decoding all samples.
fn probe_audio_metadata(path: &Path) -> Result<(u32, u16, f32), AudioError> {
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let file = std::fs::File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let format_opts = FormatOptions::default();
    let meta_opts = MetadataOptions::default();

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &format_opts, &meta_opts)
        .map_err(|e| AudioError::Decode(e.to_string()))?;

    let format = probed.format;
    let track = format
        .default_track()
        .ok_or_else(|| AudioError::Decode("no audio track".into()))?;

    let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);
    let channels = track.codec_params.channels.map(|c| c.count() as u16).unwrap_or(2);

    let duration_seconds = track
        .codec_params
        .n_frames
        .map(|n| n as f32 / sample_rate as f32)
        .unwrap_or(0.0);

    Ok((sample_rate, channels, duration_seconds))
}

/// Play an imported audio asset through the given engine.
///
/// For streaming assets, this creates a `StreamingInstance`. For in-memory
/// assets, it creates a `SoundInstance` from the decoded samples.
pub fn play_imported(
    engine: &rustix_audio::AudioEngine,
    imported: &ImportedAudio,
) -> Result<PlayResult, AudioError> {
    if imported.streaming {
        let stream = engine.stream_sound(
            Path::new(&imported.path),
            imported.volume,
            imported.looping,
        )?;
        Ok(PlayResult::Streaming(stream))
    } else {
        let instance = engine.play_sound(Path::new(&imported.path), imported.volume, imported.looping)?;
        Ok(PlayResult::Memory(instance))
    }
}

/// Result of playing an imported audio asset.
pub enum PlayResult {
    Memory(SoundInstance),
    Streaming(rustix_audio::StreamingInstance),
}
