//! Audio asset format and importer (WAV, OGG, FLAC → .rxsound).
//!
//! `.rxsound` stores decoded interleaved f32 samples ready for engine-side
//! playback, eliminating runtime decoding overhead.

use std::future::Future;
use std::pin::Pin;

use crate::handle::{Asset, AssetTypeId};
use crate::importer::{ImportResult, Importer};

// ── Audio Asset ──

/// CPU-side decoded audio data that can be serialized to `.rxsound` and later
/// played back via `AudioEngine::play_asset` or uploaded to an audio buffer.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioAsset {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
    pub duration_seconds: f32,
}

impl AudioAsset {
    pub fn new(samples: Vec<f32>, sample_rate: u32, channels: u16) -> Self {
        let frames = if channels > 0 {
            samples.len() / channels as usize
        } else {
            0
        };
        let duration_seconds = frames as f32 / sample_rate.max(1) as f32;
        Self { samples, sample_rate, channels, duration_seconds }
    }

    pub fn frame_count(&self) -> usize {
        if self.channels > 0 {
            self.samples.len() / self.channels as usize
        } else {
            0
        }
    }
}

impl Asset for AudioAsset {
    fn asset_type_id() -> AssetTypeId {
        AssetTypeId::from_crate_name("rustix_asset::AudioAsset")
    }
}

// ── .rxsound binary format ──

const RXSOUND_MAGIC: &[u8; 4] = b"RXD1";
const RXSOUND_VERSION: u32 = 1;

pub fn import_rxsound(bytes: &[u8]) -> ImportResult<AudioAsset> {
    if bytes.len() < 24 {
        return Err("rxsound: file too small for header".to_string());
    }
    if &bytes[0..4] != RXSOUND_MAGIC {
        return Err("rxsound: invalid magic".to_string());
    }

    let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    if version != RXSOUND_VERSION {
        return Err(format!("rxsound: unsupported version {version}"));
    }

    let sample_rate = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    let channels = u16::from_le_bytes([bytes[12], bytes[13]]);
    let sample_count = u64::from_le_bytes([
        bytes[14], bytes[15], bytes[16], bytes[17],
        bytes[18], bytes[19], bytes[20], bytes[21],
    ]) as usize;
    let duration_seconds = f32::from_le_bytes([bytes[22], bytes[23], bytes[24], bytes[25]]);

    let data_start = 26;
    let data_size = sample_count * 4;
    if bytes.len() < data_start + data_size {
        return Err("rxsound: file too small for sample data".to_string());
    }

    let samples: Vec<f32> = bytes[data_start..data_start + data_size]
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect();

    if samples.len() != sample_count {
        return Err("rxsound: sample count mismatch".to_string());
    }

    Ok(AudioAsset { samples, sample_rate, channels, duration_seconds })
}

pub fn export_rxsound(asset: &AudioAsset) -> Vec<u8> {
    let sample_count = asset.samples.len();
    let total = 26 + sample_count * 4;
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(RXSOUND_MAGIC);
    out.extend_from_slice(&RXSOUND_VERSION.to_le_bytes());
    out.extend_from_slice(&asset.sample_rate.to_le_bytes());
    out.extend_from_slice(&asset.channels.to_le_bytes());
    out.extend_from_slice(&(sample_count as u64).to_le_bytes());
    out.extend_from_slice(&asset.duration_seconds.to_le_bytes());
    for sample in &asset.samples {
        out.extend_from_slice(&sample.to_le_bytes());
    }
    out
}

// ── Symphonia decoder from bytes ──

fn decode_audio_bytes(bytes: &[u8], hint_ext: Option<&str>) -> ImportResult<AudioAsset> {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::DecoderOptions;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let cursor = std::io::Cursor::new(bytes.to_vec());
    let mss = MediaSourceStream::new(Box::new(cursor), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = hint_ext {
        hint.with_extension(ext);
    }

    let format_opts = FormatOptions::default();
    let meta_opts = MetadataOptions::default();
    let dec_opts = DecoderOptions::default();

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &format_opts, &meta_opts)
        .map_err(|e| format!("audio probe: {e}"))?;

    let mut format = probed.format;
    let track = format.default_track()
        .ok_or_else(|| "no audio track".to_string())?;

    let track_id = track.id;
    let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);
    let channels = track.codec_params.channels.map(|c| c.count() as u16).unwrap_or(2);

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &dec_opts)
        .map_err(|e| format!("audio decoder: {e}"))?;

    let mut samples = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(_) => break,
        };
        if packet.track_id() != track_id { continue; }

        match decoder.decode(&packet) {
            Ok(audio_buf) => {
                let spec = *audio_buf.spec();
                let mut sample_buf = SampleBuffer::<f32>::new(audio_buf.capacity() as u64, spec);
                sample_buf.copy_interleaved_ref(audio_buf);
                samples.extend_from_slice(sample_buf.samples());
            }
            Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
            Err(_) => break,
        }
    }

    if samples.is_empty() {
        return Err("no samples decoded".to_string());
    }

    Ok(AudioAsset::new(samples, sample_rate, channels))
}

// ── Importer ──

/// Importer for any audio format that symphonia supports (WAV, OGG/Vorbis, FLAC, MP3, AAC).
pub struct GenericAudioImporter;

impl Importer for GenericAudioImporter {
    type Asset = AudioAsset;

    fn name(&self) -> &'static str {
        "generic_audio"
    }

    fn extensions(&self) -> &[&'static str] {
        &["wav", "ogg", "flac", "mp3", "aac", "m4a"]
    }

    fn import<'a>(&self, bytes: &'a [u8], hint: Option<&str>) -> Pin<Box<dyn Future<Output = ImportResult<Self::Asset>> + Send + 'a>> {
        let ext = hint.and_then(|p| {
            std::path::Path::new(p)
                .extension()
                .and_then(|e| e.to_str())
        });
        Box::pin(std::future::ready(decode_audio_bytes(bytes, ext)))
    }
}

/// Importer for the native `.rxsound` binary format.
pub struct RsoundImporter;

impl Importer for RsoundImporter {
    type Asset = AudioAsset;

    fn name(&self) -> &'static str {
        "rxsound"
    }

    fn extensions(&self) -> &[&'static str] {
        &["rxsound"]
    }

    fn import<'a>(&self, bytes: &'a [u8], _hint: Option<&str>) -> Pin<Box<dyn Future<Output = ImportResult<Self::Asset>> + Send + 'a>> {
        Box::pin(std::future::ready(import_rxsound(bytes)))
    }
}
