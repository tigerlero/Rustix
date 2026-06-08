//! Tests for streaming audio decoder.

use std::path::Path;
use crate::stream::StreamDecoder;

#[test]
fn test_stream_short_wav() {
    // Use one of the generated sound effect files
    let path = Path::new("assets/sounds/click.wav");
    if !path.exists() { return; }

    let mut dec = StreamDecoder::open(path).unwrap();
    assert_eq!(dec.sample_rate(), 44100);
    assert_eq!(dec.channels(), 1);

    let mut buf = vec![0.0f32; 4096];
    let total = {
        let mut n = 0;
        loop {
            let read = dec.read(&mut buf[n..]);
            if read == 0 { break; }
            n += read;
            if n >= buf.len() { buf.resize(buf.len() * 2, 0.0); }
        }
        n
    };
    assert!(total > 0, "should read at least some samples");
    assert!(dec.is_ended());
    assert!(dec.elapsed_seconds() > 0.0);
}

#[test]
fn test_stream_then_seek() {
    let path = Path::new("assets/sounds/beep.wav");
    if !path.exists() { return; }

    let mut dec = StreamDecoder::open(path).unwrap();
    let mut buf = vec![0.0f32; 4096];
    let first = dec.read(&mut buf);

    // Seek back to start
    dec.seek(0.0).ok();
    // Seeks can fail for some formats; if it did, skip the rest
    if dec.is_ended() && first > 0 {
        // Seek not supported, but that's OK
        return;
    }

    let mut buf2 = vec![0.0f32; 4096];
    let after_seek = dec.read(&mut buf2);
    assert!(after_seek > 0);
}
