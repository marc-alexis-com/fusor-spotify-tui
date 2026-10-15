//! Short fades of the sound around pause and resume, so that neither cuts it off.
use librespot_connect::Spirc;
use librespot_playback::mixer::VolumeGetter;
use std::{
    sync::{Arc, Mutex, PoisonError},
    time::{Duration, Instant},
};

/// How long the sound takes to fade out before a pause, or in after a resume;
/// zero cuts at once.
fn fade_length() -> Duration {
    Duration::from_millis(crate::config::get().playback.fade_ms)
}

/// The share of the volume the player plays at, moving between two levels.
#[derive(Clone, Default)]
pub(crate) struct Fader(Arc<Mutex<Ramp>>);

struct Ramp {
    from: f64,
    to: f64,
    start: Instant,
    /// Changes with every fade, so that a pause waiting for its fade-out can tell
    /// that a resume came first.
    generation: u64,
}

impl Default for Ramp {
    fn default() -> Self {
        Self {
            from: 1.0,
            to: 1.0,
            start: Instant::now(),
            generation: 0,
        }
    }
}

impl Ramp {
    fn level(&self) -> f64 {
        let length = fade_length();
        if length.is_zero() {
            return self.to;
        }
        let progress = (self.start.elapsed().as_secs_f64() / length.as_secs_f64()).min(1.0);
        self.from + (self.to - self.from) * progress
    }

    fn fade(&mut self, from: f64, to: f64) -> u64 {
        *self = Self {
            from,
            to,
            start: Instant::now(),
            generation: self.generation + 1,
        };
        self.generation
    }
}

impl Fader {
    fn with<T>(&self, action: impl FnOnce(&mut Ramp) -> T) -> T {
        action(&mut self.0.lock().unwrap_or_else(PoisonError::into_inner))
    }

    fn level(&self) -> f64 {
        self.with(|ramp| ramp.level())
    }

    /// Fades the sound out, then pauses, unless a resume comes in between.
    pub(crate) fn pause(&self, spirc: &Arc<Spirc>) {
        let generation = self.with(|ramp| {
            let level = ramp.level();
            ramp.fade(level, 0.0)
        });
        let (fader, spirc) = (self.clone(), spirc.clone());
        tokio::spawn(async move {
            tokio::time::sleep(fade_length()).await;
            if fader.with(|ramp| ramp.generation) == generation {
                // Nobody listens to a failed pause; the player reports what it does.
                let _ = spirc.pause();
                // Paused, the sound is silent anyway; a resume from another app plays at once.
                fader.with(|ramp| ramp.fade(1.0, 1.0));
            }
        });
    }

    /// Resumes from silence, or from where a fade-out got to, and fades the sound in.
    pub(crate) fn resume(&self, spirc: &Spirc) -> Result<(), librespot_core::Error> {
        self.with(|ramp| {
            let from = if ramp.to < 1.0 { ramp.level() } else { 0.0 };
            ramp.fade(from, 1.0)
        });
        spirc.play()
    }

    /// The player's volume, scaled by the fade.
    pub(crate) fn wrap(
        &self,
        volume: Box<dyn VolumeGetter + Send>,
    ) -> Box<dyn VolumeGetter + Send> {
        Box::new(Faded {
            volume,
            fader: self.clone(),
        })
    }
}

struct Faded {
    volume: Box<dyn VolumeGetter + Send>,
    fader: Fader,
}

impl VolumeGetter for Faded {
    fn attenuation_factor(&self) -> f64 {
        self.volume.attenuation_factor() * self.fader.level()
    }
}

#[cfg(test)]
mod tests {
    use super::{Ramp, fade_length};
    use std::time::Instant;

    #[test]
    fn a_ramp_moves_from_one_level_to_the_other() {
        let mut ramp = Ramp::default();
        assert!((ramp.level() - 1.0).abs() < f64::EPSILON);
        let first = ramp.fade(1.0, 0.0);
        ramp.start = Instant::now() - fade_length() / 2;
        assert!((ramp.level() - 0.5).abs() < 0.05);
        ramp.start = Instant::now() - fade_length() * 2;
        assert!(ramp.level().abs() < f64::EPSILON);
        assert_ne!(ramp.fade(0.0, 1.0), first);
    }
}
