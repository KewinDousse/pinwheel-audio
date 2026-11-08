//! Volume stepping and mouse wheel accumulation.

/// Wheel rotation reported by Windows for one notch of a classic wheel.
pub const WHEEL_DELTA: i32 = 120;

/// A pause longer than this drops a partial notch, so a stray half-notch is
/// not carried over to the next gesture.
const IDLE_RESET_MS: u32 = 400;

/// Smallest and largest step accepted by [`stepped_level`], in percent.
pub const STEP_RANGE: std::ops::RangeInclusive<u8> = 1..=50;

/// Volume (0.0 to 1.0) and mute of an output.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VolumeState {
    pub level: f32,
    pub muted: bool,
}

/// Moves `current` (0.0 to 1.0) by `steps` steps of `step_percent`, landing on
/// a multiple of the step: from 33 %, one step of 10 goes up to 40 or down to 30.
pub fn stepped_level(current: f32, steps: i32, step_percent: u8) -> f32 {
    let step = i32::from(step_percent.clamp(*STEP_RANGE.start(), *STEP_RANGE.end()));
    let percent = (current.clamp(0.0, 1.0) * 100.0).round() as i32;
    let base = if steps > 0 {
        percent.div_euclid(step)
    } else {
        // Ceiling division, so that going down from 33 lands on 30.
        (percent + step - 1).div_euclid(step)
    };
    let target = ((base + steps) * step).clamp(0, 100);
    target as f32 / 100.0
}

/// Turns raw wheel deltas into whole notches.
///
/// High-resolution wheels and touchpads report fractions of
/// [`WHEEL_DELTA`]; applying one step per event would make the volume jump.
#[derive(Debug, Default)]
pub struct WheelAccumulator {
    pending: i32,
    last_time_ms: Option<u32>,
}

impl WheelAccumulator {
    /// Adds a delta received at `time_ms` (the event timestamp, which wraps
    /// around) and returns the number of complete notches, signed.
    pub fn push(&mut self, delta: i32, time_ms: u32) -> i32 {
        let idle = self
            .last_time_ms
            .is_some_and(|last| time_ms.wrapping_sub(last) > IDLE_RESET_MS);
        let reversed = self.pending != 0 && self.pending.signum() != delta.signum();
        if idle || reversed {
            self.pending = 0;
        }
        self.last_time_ms = Some(time_ms);

        self.pending += delta;
        let notches = self.pending / WHEEL_DELTA;
        self.pending -= notches * WHEEL_DELTA;
        notches
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn percent(level: f32) -> i32 {
        (level * 100.0).round() as i32
    }

    #[test]
    fn steps_land_on_multiples() {
        assert_eq!(percent(stepped_level(0.33, 1, 10)), 40);
        assert_eq!(percent(stepped_level(0.33, -1, 10)), 30);
        assert_eq!(percent(stepped_level(0.30, 1, 10)), 40);
        assert_eq!(percent(stepped_level(0.30, -1, 10)), 20);
        assert_eq!(percent(stepped_level(0.50, 3, 10)), 80);
        assert_eq!(percent(stepped_level(0.42, 1, 5)), 45);
    }

    #[test]
    fn tolerates_float_noise() {
        assert_eq!(percent(stepped_level(0.299_99, 1, 10)), 40);
        assert_eq!(percent(stepped_level(0.300_01, -1, 10)), 20);
    }

    #[test]
    fn clamps_to_bounds() {
        assert_eq!(percent(stepped_level(0.95, 1, 10)), 100);
        assert_eq!(percent(stepped_level(1.0, 1, 10)), 100);
        assert_eq!(percent(stepped_level(0.02, -1, 10)), 0);
        assert_eq!(percent(stepped_level(0.0, -2, 10)), 0);
    }

    #[test]
    fn out_of_range_step_is_clamped() {
        assert_eq!(percent(stepped_level(0.5, 1, 0)), 51);
        assert_eq!(percent(stepped_level(0.0, 1, 200)), 50);
    }

    #[test]
    fn classic_wheel_gives_one_notch_per_event() {
        let mut acc = WheelAccumulator::default();
        assert_eq!(acc.push(120, 0), 1);
        assert_eq!(acc.push(-120, 10), -1);
        assert_eq!(acc.push(240, 20), 2);
    }

    #[test]
    fn fine_deltas_add_up() {
        let mut acc = WheelAccumulator::default();
        assert_eq!(acc.push(40, 0), 0);
        assert_eq!(acc.push(40, 10), 0);
        assert_eq!(acc.push(40, 20), 1);
        assert_eq!(acc.push(40, 30), 0);
    }

    #[test]
    fn reversing_drops_the_partial_notch() {
        let mut acc = WheelAccumulator::default();
        assert_eq!(acc.push(60, 0), 0);
        assert_eq!(acc.push(-120, 10), -1);
    }

    #[test]
    fn pause_drops_the_partial_notch() {
        let mut acc = WheelAccumulator::default();
        assert_eq!(acc.push(60, 0), 0);
        assert_eq!(acc.push(60, 1_000), 0);
        assert_eq!(acc.push(60, 1_010), 1);
    }

    #[test]
    fn survives_timestamp_wraparound() {
        let mut acc = WheelAccumulator::default();
        assert_eq!(acc.push(60, u32::MAX - 5), 0);
        assert_eq!(acc.push(60, 5), 1);
    }
}
