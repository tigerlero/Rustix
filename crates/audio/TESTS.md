# Spatial Audio Tests

This document describes the test suite for the spatial audio subsystem.

## Unit Tests (9 tests - all passing)

### Distance Attenuation Tests

| Test | Description |
|------|-------------|
| `test_attenuation_at_min_distance` | Verifies full volume (1.0) when source is at or within min_distance |
| `test_attenuation_at_max_distance` | Verifies silence (0.0) when source reaches max_distance |
| `test_attenuation_between_distances` | Verifies attenuation is between 0 and 1 for distances in between |

### HRTF Panning Tests

| Test | Description |
|------|-------------|
| `test_hrtf_panning_front` | Verifies balanced left/right gains when source is directly in front |
| `test_hrtf_panning_right` | Verifies right channel dominance for sources to the right |
| `test_hrtf_panning_left` | Verifies left channel dominance for sources to the left |

### Azimuth Calculation Tests

| Test | Description |
|------|-------------|
| `test_horiz_azimuth_front` | Verifies angle 0 when source is directly in front |
| `test_horiz_azimuth_right` | Verifies +90° angle for sources to the right |
| `test_horiz_azimuth_left` | Verifies -90° angle for sources to the left |

## Integration Tests (7 tests - all passing)

### End-to-End Spatial Playback

1. **`test_spatial_playback_distance_attenuation`**
   - Creates test samples and verifies output samples are attenuated correctly

2. **`test_spatial_playback_panning`**
   - Verifies left/right channel balance matches expected gains

3. **`test_spatial_playback_combined`**
   - Tests combined distance and panning effects with no clipping

### Edge Cases

4. **`test_attenuation_negative_rolloff`**
   - Tests with negative rolloff (volume increases with distance)

5. **`test_attenuation_equal_distances`**
   - Tests when min_distance == max_distance edge case

6. **`test_panning_behind_listener`**
   - Tests sources directly behind listener (angle near ±π)

7. **`test_stereo_spatial_processing`**
   - Verifies stereo sounds are processed correctly with channel-specific gains

## Running Tests

```bash
# Run all tests
cargo test -p rustix-audio

# Run with output
cargo test -p rustix-audio -- --nocapture
```