//! Streaming audio: progressive chunked decoding for long files.

use std::fs::File;
use std::path::Path;
use std::io::{Read, Seek, SeekFrom};

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{Decoder, DecoderOptions};
use symphonia::core::errors::Error as SymphError;
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::AudioError;

/// A streaming audio decoder that reads and decodes chunks on demand.
///
/// Maintains the symphonia format reader + decoder across calls,
/// decoding the next packet when more samples are needed.
pub struct StreamDecoder {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,
    sample_rate: u32,
    channels: u16,
    /// Buffered decoded samples not yet consumed
    buffer: Vec<f32>,
    /// Position in the buffer
    buf_pos: usize,
    /// Total frames decoded so far
    total_frames: u64,
    /// Whether the stream has ended
    ended: bool,
}

impl StreamDecoder {
    /// Open a file and prepare for streaming.
    pub fn open(path: &Path) -> Result<Self, AudioError> {
        let file = File::open(path)?;
        let mss = MediaSourceStream::new(Box::new(FileSource::new(file)), Default::default());

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

        let format = probed.format;
        let track = format.default_track()
            .ok_or_else(|| AudioError::Decode("no audio track".into()))?;

        let track_id = track.id;
        let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);
        let channels = track.codec_params.channels.map(|c| c.count() as u16).unwrap_or(2);

        let decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &dec_opts)
            .map_err(|e| AudioError::Decode(e.to_string()))?;

        Ok(Self {
            format,
            decoder,
            track_id,
            sample_rate,
            channels,
            buffer: Vec::new(),
            buf_pos: 0,
            total_frames: 0,
            ended: false,
        })
    }

    /// Sample rate in Hz.
    pub fn sample_rate(&self) -> u32 { self.sample_rate }

    /// Number of channels (1 = mono, 2 = stereo).
    pub fn channels(&self) -> u16 { self.channels }

    /// Total frames decoded so far.
    pub fn total_frames(&self) -> u64 { self.total_frames }

    /// Total seconds decoded so far.
    pub fn elapsed_seconds(&self) -> f32 {
        self.total_frames as f32 / self.sample_rate as f32
    }

    /// Whether the stream has ended (no more packets).
    pub fn is_ended(&self) -> bool { self.ended }

    /// Read the next `max_samples` interleaved f32 samples.
    ///
    /// Returns the number of samples actually read (may be less than max_samples
    /// at end of stream, or 0 if already ended).
    pub fn read(&mut self, output: &mut [f32]) -> usize {
        if self.ended && self.buf_pos >= self.buffer.len() {
            return 0;
        }

        let mut written = 0usize;

        // Drain existing buffer first
        while self.buf_pos < self.buffer.len() && written < output.len() {
            output[written] = self.buffer[self.buf_pos];
            self.buf_pos += 1;
            written += 1;
        }

        // Decode more packets while we need samples
        while written < output.len() && !self.ended {
            match self.decode_next_packet() {
                Ok(0) => {
                    // No more packets
                    self.ended = true;
                    break;
                }
                Ok(decoded) => {
                    // Fill output from buffer
                    while self.buf_pos < self.buffer.len() && written < output.len() {
                        output[written] = self.buffer[self.buf_pos];
                        self.buf_pos += 1;
                        written += 1;
                    }
                    let _ = decoded;
                }
                Err(_) => {
                    self.ended = true;
                    break;
                }
            }
        }

        written
    }

    /// Seek to approximately the given time in seconds.
    ///
    /// This is approximate because symphonia's format reader seek is
    /// not always frame-accurate for all codecs.
    pub fn seek(&mut self, seconds: f32) -> Result<(), AudioError> {
        let target_frames = (seconds * self.sample_rate as f32) as u64;

        let seek_result = self.format.seek(
            symphonia::core::formats::SeekMode::Accurate,
            symphonia::core::formats::SeekTo::TimeStamp {
                ts: target_frames,
                track_id: self.track_id,
            },
        );

        match seek_result {
            Ok(seeked_to) => {
                self.decoder.reset();
                self.buffer.clear();
                self.buf_pos = 0;
                self.ended = false;
                self.total_frames = seeked_to.actual_ts;
                Ok(())
            }
            Err(SymphError::SeekError(_)) => {
                Err(AudioError::Decode("seek not supported for this format".into()))
            }
            Err(e) => Err(AudioError::Decode(e.to_string())),
        }
    }

    /// Decode the next packet into the internal buffer.
    /// Returns the number of samples decoded, or 0 if stream ended.
    fn decode_next_packet(&mut self) -> Result<usize, AudioError> {
        let packet = loop {
            match self.format.next_packet() {
                Ok(p) => break p,
                Err(SymphError::IoError(ref e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    return Ok(0);
                }
                Err(SymphError::DecodeError(_)) => continue,
                Err(_) => return Ok(0),
            }
        };

        if packet.track_id() != self.track_id {
            return Ok(0);
        }

        let decoded = match self.decoder.decode(&packet) {
            Ok(audio) => audio,
            Err(SymphError::DecodeError(_)) => return Ok(1), // skip bad packet, keep going
            Err(_) => return Ok(0),
        };

        let spec = *decoded.spec();
        let frames = decoded.frames() as usize;
        let mut sample_buf = SampleBuffer::<f32>::new(frames as u64, spec);
        sample_buf.copy_interleaved_ref(decoded);

        let new_samples = sample_buf.samples();
        self.buffer.drain(..self.buf_pos);
        self.buf_pos = 0;
        self.buffer.extend_from_slice(new_samples);
        self.total_frames += frames as u64;

        Ok(new_samples.len())
    }
}

/// A `Read + Seek` wrapper over `File` that implements `MediaSource` for symphonia.
struct FileSource {
    file: File,
    pos: u64,
}

impl FileSource {
    fn new(file: File) -> Self {
        Self { file, pos: 0 }
    }
}

impl Read for FileSource {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.file.read(buf)?;
        self.pos += n as u64;
        Ok(n)
    }
}

impl Seek for FileSource {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let new_pos = self.file.seek(pos)?;
        self.pos = new_pos;
        Ok(new_pos)
    }
}

impl symphonia::core::io::MediaSource for FileSource {
    fn is_seekable(&self) -> bool { true }
    fn byte_len(&self) -> Option<u64> {
        self.file.metadata().ok().map(|m| m.len())
    }
}
