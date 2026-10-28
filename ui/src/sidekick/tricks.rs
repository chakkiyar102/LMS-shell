//! Tapping Sidekick: one tap plays a trick from the pool (never the same one twice running), three quick
//! taps make him dizzy, seven quick taps play the secret move. Pure, tested natively.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trick { Spin, Flip, Wave, Wink, Salute, Hop, Loop, Peek }

pub const TRICKS: [Trick; 8] = [Trick::Spin, Trick::Flip, Trick::Wave, Trick::Wink, Trick::Salute, Trick::Hop, Trick::Loop, Trick::Peek];

impl Trick {
    /// The picture he shows while doing it (ui/assets/mascot/<pose>.png).
    pub fn pose(self) -> &'static str {
        match self {
            Trick::Spin => "hop", Trick::Flip => "celebrate", Trick::Wave => "wave", Trick::Wink => "wink",
            Trick::Salute => "salute", Trick::Hop => "hop", Trick::Loop => "fly", Trick::Peek => "peek",
        }
    }
    /// The CSS animation class on the sprite's inner wrapper (ui/sidekick.css), transform only.
    pub fn class(self) -> &'static str {
        match self {
            Trick::Spin => "t-spin", Trick::Flip => "t-flip", Trick::Wave => "t-wave", Trick::Wink => "t-wink",
            Trick::Salute => "t-salute", Trick::Hop => "t-hop", Trick::Loop => "t-loop", Trick::Peek => "t-peek",
        }
    }
    /// How long the move runs; the pose goes back to idle after it.
    pub fn ms(self) -> u32 {
        match self { Trick::Loop => 1100, Trick::Spin | Trick::Flip => 800, Trick::Hop => 650, _ => 1200 }
    }
    /// The line context (sidekick/lines.rs).
    pub fn lines(self) -> &'static str {
        match self {
            Trick::Spin => "trick_spin", Trick::Flip => "trick_flip", Trick::Wave => "trick_wave", Trick::Wink => "trick_wink",
            Trick::Salute => "trick_salute", Trick::Hop => "trick_hop", Trick::Loop => "trick_loop", Trick::Peek => "trick_peek",
        }
    }
}

/// A trick other than `last`, chosen by `r`.
pub fn next_trick(last: Option<Trick>, r: u32) -> Trick {
    let pool: Vec<Trick> = TRICKS.into_iter().filter(|t| Some(*t) != last).collect();
    pool[r as usize % pool.len()]
}

/// Taps closer together than this count as one burst.
pub const QUICK_MS: f64 = 650.0;
pub const DIZZY_AT: u32 = 3;
pub const SECRET_AT: u32 = 7;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tap {
    /// The first tap of a burst: a trick, and the Where to? menu opens.
    First,
    /// A second quick tap: another trick, the menu stays as it is.
    Again,
    Dizzy,
    Secret,
    /// Still dizzy: nothing new until the secret move or the burst ends.
    Ignore,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Taps { count: u32, last: Option<f64> }

impl Taps {
    pub fn tap(&mut self, now: f64) -> Tap {
        let quick = self.last.is_some_and(|t| now - t >= 0.0 && now - t < QUICK_MS);
        self.count = if quick { self.count + 1 } else { 1 };
        self.last = Some(now);
        match self.count {
            1 => Tap::First,
            n if n < DIZZY_AT => Tap::Again,
            DIZZY_AT => Tap::Dizzy,
            SECRET_AT => { self.count = 0; self.last = None; Tap::Secret }
            _ => Tap::Ignore,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_trick_pool_never_repeats() {
        let mut last = None;
        let mut seen = std::collections::BTreeSet::new();
        let mut r = crate::sidekick::lines::Rng::new(42);
        for _ in 0..400 {
            let t = next_trick(last, r.next());
            assert_ne!(Some(t), last);
            seen.insert(t.class());
            last = Some(t);
        }
        assert_eq!(seen.len(), TRICKS.len(), "every trick turns up");
        for t in TRICKS { assert!(t.ms() >= 500 && !crate::sidekick::lines::lines(t.lines()).is_empty()) }
    }

    #[test]
    fn three_quick_taps_dizzy_seven_secret_slow_taps_reset() {
        let mut t = Taps::default();
        assert_eq!(t.tap(0.0), Tap::First);
        assert_eq!(t.tap(300.0), Tap::Again);
        assert_eq!(t.tap(600.0), Tap::Dizzy);
        assert_eq!(t.tap(900.0), Tap::Ignore);
        assert_eq!(t.tap(1200.0), Tap::Ignore);
        assert_eq!(t.tap(1500.0), Tap::Ignore);
        assert_eq!(t.tap(1800.0), Tap::Secret);
        assert_eq!(t.tap(1900.0), Tap::First, "the burst starts over after the secret");
        let mut s = Taps::default();
        for i in 0..6 { assert_eq!(s.tap(i as f64 * 2000.0), Tap::First, "slow taps never make him dizzy") }
        let mut u = Taps::default();
        u.tap(0.0); u.tap(400.0);
        assert_eq!(u.tap(1400.0), Tap::First, "a pause breaks the burst");
    }
}
