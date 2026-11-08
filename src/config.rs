pub const DEFAULT_PORT: u16 = 9697;
pub const DEFAULT_HOST: &str = "0.0.0.0";
pub const SAMPLE_RATE: u32 = 48000;
pub const CHANNELS: u16 = 2;
// 960 frames * 4 bytes per sample (16-bit stereo) = 3840 bytes (~20ms of audio)
pub const CHUNK_SIZE: usize = 3840;
pub const BROADCAST_CAPACITY: usize = 64;
