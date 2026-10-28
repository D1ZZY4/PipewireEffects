pub fn soft_knee_reduction_db(level_db: f32, threshold_db: f32, knee_db: f32, ratio: f32) -> f32 {
    if !level_db.is_finite() {
        return 0.0;
    }
    let over = level_db - threshold_db;
    let slope = 1.0 - 1.0 / ratio.max(1.0);
    if knee_db <= 0.0 {
        if over > 0.0 { -over * slope } else { 0.0 }
    } else if over <= -0.5 * knee_db {
        0.0
    } else if over >= 0.5 * knee_db {
        -over * slope
    } else {
        let shifted = over + 0.5 * knee_db;
        -(slope * shifted * shifted / (2.0 * knee_db))
    }
}

pub fn ballistics(attack_ms: f32, release_ms: f32, rate: u32) -> (f32, f32) {
    let rate = rate.max(1) as f32;
    let attack = 1.0 - (-1.0 / ((attack_ms.max(0.01) / 1000.0) * rate)).exp();
    let release = 1.0 - (-1.0 / ((release_ms.max(0.01) / 1000.0) * rate)).exp();
    (attack.clamp(0.0, 1.0), release.clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn below_the_knee_there_is_no_reduction() {
        assert_eq!(soft_knee_reduction_db(-40.0, -18.0, 6.0, 4.0), 0.0);
    }

    #[test]
    fn above_the_knee_matches_the_ratio() {
        let reduction = soft_knee_reduction_db(0.0, -18.0, 0.0, 4.0);
        let expected = -18.0 * 0.75;
        assert!((reduction - expected).abs() < 1.0e-4, "{reduction}");
    }

    #[test]
    fn the_knee_is_continuous() {
        let threshold = -18.0;
        let knee = 6.0;
        let ratio = 4.0;
        let below = soft_knee_reduction_db(threshold - knee * 0.5 - 1.0e-4, threshold, knee, ratio);
        let above = soft_knee_reduction_db(threshold + knee * 0.5 + 1.0e-4, threshold, knee, ratio);
        assert!(below.abs() < 1.0e-3);
        assert!((above - (-knee * 0.5 * 0.75)).abs() < 1.0e-2);
    }

    #[test]
    fn ballistics_reach_their_targets() {
        let (attack, release) = ballistics(10.0, 100.0, 48_000);
        assert!(attack > release);
        let mut envelope = 0.0f32;
        for _ in 0..48_000 {
            let target = -6.0;
            let coefficient = if target < envelope { attack } else { release };
            envelope += (target - envelope) * coefficient;
        }
        assert!((envelope + 6.0).abs() < 0.01);
    }
}
