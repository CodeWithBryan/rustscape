//! Revision-530 client XP thresholds. Server skill rules remain authoritative.
pub fn thresholds() -> &'static [i32; 99] {
    static LEVELS: std::sync::OnceLock<[i32; 99]> = std::sync::OnceLock::new();
    LEVELS.get_or_init(|| {
        let mut total = 0i32;
        std::array::from_fn(|i| {
            let level = (i + 1) as f64;
            total = total.wrapping_add((2f64.powf(level / 7.0) * 300.0 + level) as i32);
            total / 4
        })
    })
}
pub fn base_level(experience: i32) -> i32 {
    let mut level = 1;
    for (i, &threshold) in thresholds().iter().take(98).enumerate() {
        if threshold <= experience {
            level = i as i32 + 2;
        }
    }
    level
}
