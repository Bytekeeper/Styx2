pub struct Duration {
    frames: i32,
}

impl Duration {
    pub fn from_frames(frames: i32) -> Self {
        Self { frames }
    }

    pub fn from_duration(duration: std::time::Duration) -> Self {
        Self {
            frames: (duration.as_millis() / 42) as i32,
        }
    }
}
