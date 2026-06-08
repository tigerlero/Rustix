use std::fs::File;
use std::path::Path;

use tracing::debug;

use crate::AudioError;

/// Convert a `rustix_asset::audio::AudioAsset` into the runtime audio tuple.
pub fn decode_from_asset(asset: &rustix_asset::audio::AudioAsset) -> (Vec<f32>, u32, u16) {
    (asset.samples.clone(), asset.sample_rate, asset.channels)
}

pub fn decode_audio(path: &Path) -> Result<(Vec<f32>, u32, u16), AudioError> {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::DecoderOptions;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let file = File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let format_opts = FormatOptions::default();
    let meta_opts = MetadataOptions::default();
    let dec_opts = DecoderOptions::default();

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &format_opts, &meta_opts)
        .map_err(|e| AudioError::Decode(e.to_string()))?;

    let mut format = probed.format;
    let track = format.default_track()
        .ok_or_else(|| AudioError::Decode("no audio track".into()))?;

    let track_id = track.id;
    let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);
    let channels = track.codec_params.channels.map(|c| c.count() as u16).unwrap_or(2);

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &dec_opts)
        .map_err(|e| AudioError::Decode(e.to_string()))?;

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
        return Err(AudioError::Decode("no samples decoded".into()));
    }

    debug!("decoded {}: {} samples, {}Hz, {}ch", path.display(), samples.len(), sample_rate, channels);
    Ok((samples, sample_rate, channels))
}