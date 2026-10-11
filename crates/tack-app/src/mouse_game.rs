//! Local screen-space toy. Injected time; one bounded clock, no document access.
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};
pub mod artwork;
#[cfg(test)]
mod tests;
pub const STEP: Duration = Duration::from_millis(33);
pub const SPAWN: Duration = Duration::from_secs(3);
pub const DWELL: Duration = Duration::from_secs(4);
pub const FRAME: Duration = Duration::from_nanos(1_000_000_000 / 12);
pub const ATTACK_FRAME: Duration = FRAME;
pub const MUNCH_FRAME: Duration = FRAME;
pub const EXPLOSION_FRAME: Duration = FRAME;
pub const MUNCH: Duration = Duration::from_secs(3);
pub const AFTER: Duration = Duration::from_secs(1);
pub const SMOKE_FRAME: Duration = FRAME;
pub const SMOKE_LIMIT: usize = 24;
pub const MAX_SPEED: f64 = 360.;
pub const JITTER: f64 = 3.5;
// Authored 16px canvases at x2; native cheese hotspot (7,7) at x2.
pub const PIXEL_SCALE: f64 = 2.;
pub const SPRITE_WIDTH: f64 = 16. * PIXEL_SCALE;
pub const CHEESE_HOTSPOT: f64 = 7. * PIXEL_SCALE;
pub const SIDE_GAP: f64 = PIXEL_SCALE;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    SpawnDelay,
    Run,
    Idle,
    Attack,
    Munch,
    Explosion,
    PostExplosionWait,
    Finished,
}
#[derive(Clone, Copy, Debug)]
pub struct Puff {
    pub at: [f64; 2],
    pub b: bool,
    pub flip: bool,
    pub frame: usize,
    next: Instant,
}
pub struct Game {
    pub state: State,
    pub at: [f64; 2],
    pub velocity: [f64; 2],
    pub frame: usize,
    pub flip: bool,
    pub hidden: bool,
    pub puffs: VecDeque<Puff>,
    pub advances: u32,
    target: [f64; 2],
    dock_right: bool,
    saved: [f64; 2],
    reference: [f64; 2],
    dwell: Option<Instant>,
    next: Instant,
    animation: Instant,
    entered: Instant,
    last: Instant,
    distance: f64,
    smoke_b: bool,
    seed: u64,
    presentation_required: bool,
    presented: bool,
}
fn distance(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}
impl Game {
    pub fn new(now: Instant, target: [f64; 2], seed: u64) -> Self {
        Self {
            state: State::SpawnDelay,
            at: target,
            velocity: [0.; 2],
            frame: 0,
            flip: false,
            hidden: false,
            puffs: VecDeque::with_capacity(SMOKE_LIMIT),
            advances: 0,
            target,
            dock_right: false,
            saved: target,
            reference: target,
            dwell: None,
            next: now + SPAWN,
            animation: now,
            entered: now,
            last: now,
            distance: 0.,
            smoke_b: false,
            seed,
            presentation_required: false,
            presented: true,
        }
    }
    pub fn require_presentation(&mut self) {
        self.presentation_required = true;
    }
    pub fn presented(&mut self) {
        self.presented = true;
    }
    pub fn pointer(&mut self, at: [f64; 2]) {
        if at.into_iter().all(f64::is_finite) {
            if self.state == State::Idle && at != self.target {
                self.next = self.last;
            }
            self.target = at;
        }
    }
    pub fn deadline(&self) -> Option<Instant> {
        (self.state != State::Finished).then_some(self.next)
    }
    fn enter(&mut self, state: State, now: Instant) {
        self.state = state;
        self.frame = 0;
        self.entered = now;
        self.animation = now;
        self.last = now;
        self.next = now
            + match state {
                State::Attack => ATTACK_FRAME,
                State::Munch => MUNCH_FRAME,
                State::Explosion => EXPLOSION_FRAME,
                State::PostExplosionWait => AFTER,
                _ => STEP,
            };
    }
    fn spawn(&mut self, size: [f64; 2], now: Instant) {
        let t = ((self.seed >> 2) % 1000) as f64 / 999.;
        let [w, h] = size.map(|v| v.clamp(1., 16384.));
        self.at = match self.seed % 4 {
            0 => [-32., t * h],
            1 => [w + 32., t * h],
            2 => [t * w, -32.],
            _ => [t * w, h + 32.],
        };
        self.dock_right = self.at[0] > self.target[0];
        self.fit_side(size[0]);
        self.flip = self.dock_target()[0] > self.at[0];
        self.enter(State::Run, now);
    }
    /// One visible animation frame per wake; late events stretch instead of skipping art.
    pub fn tick(&mut self, now: Instant, size: [f64; 2]) -> bool {
        if self.deadline().is_none_or(|next| now < next) {
            return false;
        }
        if self.presentation_required
            && !self.presented
            && matches!(self.state, State::Attack | State::Munch | State::Explosion)
        {
            self.next = now + STEP;
            return false;
        }
        match self.state {
            State::SpawnDelay => self.spawn(size, now),
            State::Run | State::Idle => self.chase(now, size),
            State::Attack => {
                if self.frame < 14 {
                    self.frame += 1;
                    self.next = now + ATTACK_FRAME;
                    if self.frame == 10 {
                        self.hidden = true;
                        self.advances += 1;
                        // Land on the frozen cheese center once; never change the ground height.
                        self.at[0] = self.saved[0];
                    }
                } else {
                    self.enter(State::Munch, now);
                }
            }
            State::Munch => {
                if now.saturating_duration_since(self.entered) >= MUNCH {
                    self.enter(State::Explosion, now);
                } else {
                    self.frame ^= 1;
                    self.next = (now + MUNCH_FRAME).min(self.entered + MUNCH);
                }
            }
            State::Explosion => {
                if self.frame < 11 {
                    self.frame += 1;
                    self.next = now + EXPLOSION_FRAME;
                } else {
                    self.enter(State::PostExplosionWait, now);
                }
            }
            State::PostExplosionWait => {
                self.state = State::Finished;
                self.hidden = false;
                self.puffs.clear();
            }
            State::Finished => return false,
        }
        self.presented = false;
        true
    }
    pub fn dock_target(&self) -> [f64; 2] {
        let half = SPRITE_WIDTH / 2.;
        let x = if self.dock_right {
            SPRITE_WIDTH - CHEESE_HOTSPOT + SIDE_GAP + half
        } else {
            -CHEESE_HOTSPOT - SIDE_GAP - half
        };
        [
            self.target[0] + x,
            self.target[1] + SPRITE_WIDTH - CHEESE_HOTSPOT,
        ]
    }
    fn fit_side(&mut self, width: f64) {
        let [x, _] = self.dock_target();
        if self.dock_right && x + SPRITE_WIDTH / 2. > width {
            self.dock_right = false;
        } else if !self.dock_right && x - SPRITE_WIDTH / 2. < 0. {
            self.dock_right = true;
        }
    }
    fn chase(&mut self, now: Instant, size: [f64; 2]) {
        let dt = now
            .saturating_duration_since(self.last)
            .as_secs_f64()
            .min(0.050);
        self.last = now;
        self.next = now + STEP;
        self.fit_side(size[0]);
        let dock = self.dock_target();
        if self.state == State::Idle {
            if distance(self.target, self.reference) > JITTER || distance(self.at, dock) > 24. {
                self.dwell = None;
                self.enter(State::Run, now);
            } else if self
                .dwell
                .is_some_and(|at| now.saturating_duration_since(at) >= DWELL)
            {
                self.saved = [
                    self.target[0] + SPRITE_WIDTH / 2. - CHEESE_HOTSPOT,
                    self.target[1],
                ];
                self.velocity = [0.; 2];
                self.puffs.clear();
                self.enter(State::Attack, now);
                return;
            }
        }
        if self.state == State::Run {
            let previous = self.at;
            let delta = [dock[0] - self.at[0], dock[1] - self.at[1]];
            for (i, d) in delta.into_iter().enumerate() {
                self.velocity[i] =
                    (self.velocity[i] + d.clamp(-160., 160.) * 9. * dt) * (-4.5 * dt).exp();
            }
            let speed = self.velocity[0].hypot(self.velocity[1]);
            if speed > MAX_SPEED {
                self.velocity = self.velocity.map(|v| v * MAX_SPEED / speed);
            }
            if self.velocity[0].abs() > 8. {
                self.flip = self.velocity[0] > 0.;
            }
            for (i, v) in self.velocity.into_iter().enumerate() {
                self.at[i] = (self.at[i] + v * dt).clamp(-64., size[i].clamp(1., 16384.) + 64.);
            }
            self.smoke(previous, now);
            if distance(self.at, dock) < 3.
                && self.velocity[0].hypot(self.velocity[1]) < 24.
                && now.saturating_duration_since(self.entered) >= Duration::from_millis(250)
            {
                self.velocity = [0.; 2];
                self.at = dock;
                self.flip = !self.dock_right;
                self.reference = self.target;
                self.dwell = Some(now);
                self.enter(State::Idle, now);
            }
        }
        if self.state == State::Idle {
            self.at = dock;
            self.flip = !self.dock_right;
        }
        let interval = FRAME;
        if now.saturating_duration_since(self.animation) >= interval {
            self.frame = (self.frame + 1) % 3;
            self.animation = now;
        }
        self.puffs.retain_mut(|p| {
            if now >= p.next {
                p.frame += 1;
                p.next = now + SMOKE_FRAME;
            }
            p.frame < 4
        });
        if self.state == State::Run {
            self.next = self.next.min(self.animation + FRAME);
            if let Some(next) = self.puffs.iter().map(|p| p.next).min() {
                self.next = self.next.min(next);
            }
        }
        // Idle sprites still animate, but wake only at a visible change/transition.
        if self.state == State::Idle {
            self.next =
                (self.animation + interval).min(self.dwell.map_or(now + interval, |at| at + DWELL));
            if let Some(next) = self.puffs.iter().map(|p| p.next).min() {
                self.next = self.next.min(next);
            }
        }
    }
    fn smoke(&mut self, previous: [f64; 2], now: Instant) {
        if self.velocity[0].hypot(self.velocity[1]) < 80. {
            return;
        }
        self.distance += distance(previous, self.at);
        if self.distance < 20. {
            return;
        }
        self.distance %= 20.;
        if self.puffs.len() == SMOKE_LIMIT {
            self.puffs.pop_front();
        }
        self.puffs.push_back(Puff {
            at: previous,
            b: self.smoke_b,
            flip: !self.flip,
            frame: 0,
            next: now + SMOKE_FRAME,
        });
        self.smoke_b = !self.smoke_b;
    }
}
