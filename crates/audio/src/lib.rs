pub mod effects;
pub use effects::{AudioEffect, EffectChain, Compressor, Equalizer, Reverb};

pub mod stream;
pub use stream::StreamDecoder;

pub mod spatial;
pub use spatial::{AudioListener, AudioSource, calculate_attenuation, hrtf_panning, calculate_horiz_azimuth, process_spatial};
pub use spatial::{REFERENCE_DISTANCE, SPEED_OF_SOUND, HEAD_RADIUS, MAX_ITD};

pub mod types;
pub use types::{SoundId, SoundInstance, SoundPlayer, AudioError};

pub mod decoder;

pub mod engine;
pub use engine::{AudioEngine, StreamingInstance};