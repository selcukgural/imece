//! What is drawn when: the frame's timing and what moves on the screen.

use super::*;

impl App {
    /// Whether the screen is drawn at `now`: on a change, never on a
    /// timer. At once when something came from the terminal since the
    /// last drawing; when something else changed or the screen's clock
    /// moved ([`Clock`]), at once while the terminal has the focus or
    /// never says (no focus reports), else [`BLURRED_DRAW`] after the
    /// last drawing at the soonest.
    pub fn draw_due(&self, now: Instant) -> bool {
        if self.stirred {
            return true;
        }
        let moved = self.changed
            || self.drawn_clock != Some(self.clock(now))
            || self.drawn_motion.is_some_and(|at| now >= at);
        moved
            && (self.blurred.is_none()
                || (self.drawn_at).is_none_or(|at| now.duration_since(at) >= BLURRED_DRAW))
    }

    /// The screen was drawn at `now`.
    pub fn drawn(&mut self, now: Instant) {
        self.drawn_at = Some(now);
        self.drawn_clock = Some(self.clock(now));
        self.stirred = false;
        self.changed = false;
        // A glow not yet begun (it waits for its beat) is kept.
        self.glows
            .retain(|&(_, glow, at)| now.saturating_duration_since(at) < glow.lasts());
        self.drawn_motion = self.next_motion(now);
    }

    /// What the screen shows of time at `now`.
    fn clock(&self, now: Instant) -> Clock {
        // A view's flash is the one on the screen.
        let shown = self.viewing.as_ref().map_or(&self.flash, |v| &v.flash);
        let flash = (shown.as_ref())
            .filter(|(_, at)| now.saturating_duration_since(*at) < FLASH)
            .map(|(_, at)| *at);
        let grace = self.grace_left(now).map(|(_, left)| left);
        (self.world.tick, flash, grace)
    }

    pub(super) fn set_motion(&mut self, how: Motion) {
        self.motion = how;
        let t = self.lang.text();
        self.flash(match (how, how.under(self.ssh)) {
            (Motion::Off, _) => t.motion_off,
            (Motion::Calm, _) => t.motion_calm,
            (Motion::Full, Motion::Full) => t.motion_full,
            (Motion::Full, _) => t.motion_ssh,
        });
    }

    /// What moves now: nothing in another window, and over SSH no more
    /// than calm.
    pub fn moving(&self) -> Motion {
        if self.blurred.is_some() {
            return Motion::Off;
        }
        self.motion.under(self.ssh)
    }

    /// How far into motion's round the frame being drawn is.
    fn motion_age(&self) -> std::time::Duration {
        self.frame.saturating_duration_since(self.epoch)
    }

    /// Whether a working row's glyph shows its second tone in this frame.
    pub fn breath(&self) -> bool {
        self.moving() != Motion::Off && motion::frame(self.motion_age(), motion::BREATH, 2) == 1
    }

    /// The dots after `starting` in this frame; none when nothing moves.
    pub fn dots(&self) -> Option<&'static str> {
        (self.moving() != Motion::Off).then(|| {
            let at = motion::frame(self.motion_age(), motion::DOTS, motion::DOT_FRAMES.len());
            motion::DOT_FRAMES[at]
        })
    }

    /// The mark is in the drawing being made: when the one before had
    /// none, its dots start coming on the next beat.
    pub fn mark_shown(&self) {
        if !self.mark_before.get() && !self.mark_drawn.get() {
            let since = self.frame.saturating_duration_since(self.epoch);
            self.mark_from
                .set(Some(self.epoch + motion::on_beat(since)));
        }
        self.mark_drawn.set(true);
    }

    /// How many of the mark's dots this frame shows: one more each beat
    /// from when it came on the screen until all are there; all at once
    /// after a key, with motion off, in another window or over SSH.
    pub fn mark_dots(&self) -> usize {
        let Some(from) = self.mark_from.get() else {
            return ui::brand::DOTS;
        };
        if self.ssh || self.moving() == Motion::Off {
            return ui::brand::DOTS;
        }
        let beats = self.frame.saturating_duration_since(from).as_nanos() / motion::BEAT.as_nanos();
        usize::try_from(beats).map_or(ui::brand::DOTS, |b| b.min(ui::brand::DOTS))
    }

    /// The spinner's frame where imece waits for an answer; none when
    /// nothing moves.
    pub fn spin(&self) -> Option<&'static str> {
        let period = self.moving().spin()?;
        Some(motion::SPIN[motion::frame(self.motion_age(), period, motion::SPIN.len())])
    }

    /// The spinner by the header's fleet: full motion only, while anyone
    /// is mid-turn.
    pub fn head_spin(&self) -> Option<&'static str> {
        (self.moving() == Motion::Full && !self.world.mid_turn().is_empty()).then(|| {
            motion::SPIN[motion::frame(self.motion_age(), motion::SPIN_FULL, motion::SPIN.len())]
        })
    }

    /// How `on` lights in this frame, if it does.
    pub fn lit(&self, on: GlowOn) -> Option<Lit> {
        if self.moving() == Motion::Off {
            return None;
        }
        let &(_, glow, at) = self.glows.iter().find(|g| g.0 == on)?;
        glow.lit(self.frame.saturating_duration_since(at))
    }

    /// What imece waits for an answer to (`:commit`), while it waits.
    pub fn asking(&self) -> Option<&str> {
        self.asking.as_deref()
    }

    /// Whether the bottom line shows the spinner at `now`: imece waits for
    /// an answer and nothing else is said there.
    fn footer_spins(&self, now: Instant) -> bool {
        self.asking.is_some()
            && !self.typing
            && (self.flash.as_ref())
                .is_none_or(|(_, at)| now.saturating_duration_since(*at) >= FLASH)
            && self.grace_left(now).is_none()
    }

    /// What moves on the screen at `now`, each with how often it moves:
    /// nothing when nothing moves (motion off, another window, an
    /// archive's view). Glows are apart ([`App::glows`]).
    fn moving_parts(&self, now: Instant) -> Vec<(Moves, Duration)> {
        let moving = self.moving();
        if moving == Motion::Off || self.viewing.is_some() {
            return Vec::new();
        }
        let w = &self.world;
        let mut parts = Vec::new();
        if self.zoom == Zoom::None && self.window.is_none() {
            let rungs = self.rungs();
            let rows = self.home_rows();
            let shown = rows.iter().filter_map(|r| match r.1 {
                HomeRow::Agent(id) => Some(id),
                _ => None,
            });
            if shown
                .into_iter()
                .any(|id| ui::breathes(&w.agents[id], rungs[id]))
            {
                parts.push((Moves::Breath, motion::BREATH));
            }
        }
        // The board's window shows the owners of the cards taken.
        if self.window == Some(Window::Board) {
            let rungs = self.rungs();
            let owners = (w.shown_cards())
                .filter(|c| c.column != Column::Waiting)
                .filter_map(|c| c.owner);
            if owners
                .map(|id| &w.agents[id])
                .any(|a| !a.gone && ui::breathes(a, rungs[a.id]))
            {
                parts.push((Moves::Breath, motion::BREATH));
            }
        }
        let checking = self.welcome && self.seen == Seen::Checking;
        if let Some(spin) = moving.spin()
            && (self.footer_spins(now) || checking)
        {
            parts.push((Moves::Spin, spin));
        }
        if moving == Motion::Full && !w.mid_turn().is_empty() {
            parts.push((Moves::HeadSpin, motion::SPIN_FULL));
        }
        // The mark still coming, where the last drawing showed it; as
        // `mark_dots` counts at `now` (its start on a beat).
        if let Some(from) = self.mark_from.get()
            && self.mark_drawn.get()
            && !self.ssh
            && now < from + motion::BEAT * ui::brand::DOTS as u32
        {
            parts.push((Moves::Mark, motion::BEAT));
        }
        parts
    }

    /// When what moves on the screen next looks different, after `now`:
    /// none when nothing moves (motion off, another window, an archive's
    /// view, nothing on the screen that moves, every glow over).
    pub fn next_motion(&self, now: Instant) -> Option<Instant> {
        if self.moving() == Motion::Off || self.viewing.is_some() {
            return None;
        }
        let parts = self.moving_parts(now);
        let since = now.saturating_duration_since(self.epoch);
        // A glow that starts on the next beat is lit from now until the
        // beat after it.
        let glows = (self.glows.iter()).filter_map(|&(_, glow, at)| {
            (glow.next(now.saturating_duration_since(at))).map(|after| now.max(at) + after)
        });
        (parts.into_iter())
            .map(|(_, p)| now + motion::next_frame(since, p))
            .chain(glows)
            .min()
    }

    /// Whether the frame due at `now` may be drawn as the last drawing
    /// with only its breath changed (`ui::patch`): nothing came from the
    /// terminal, nothing changed, the clock shows the same, the breath is
    /// all that moves (no glow, dots or spinner), and that drawing knew
    /// where every glyph that breathes is.
    pub fn patch_due(&self, now: Instant) -> bool {
        !self.stirred
            && !self.changed
            && self.drawn_clock == Some(self.clock(now))
            && self.glows.is_empty()
            && self.moving_parts(now) == [(Moves::Breath, motion::BREATH)]
            && self.patchable()
    }

    /// Whether the last drawing can be patched: it knew where every glyph
    /// that breathes is, and some do.
    pub fn patchable(&self) -> bool {
        self.breathing
            .borrow()
            .as_ref()
            .is_some_and(|b| !b.is_empty())
    }

    /// The frame drawn `after` motion's start (tests).
    #[cfg(test)]
    pub fn frame_after(&mut self, after: Duration) {
        self.frame = self.epoch + after;
    }

    /// The moment `after` motion's start (tests).
    #[cfg(test)]
    pub fn after(&self, after: Duration) -> Instant {
        self.epoch + after
    }

    /// `on` lights with `glow` from the frame's beat (tests).
    #[cfg(test)]
    pub fn light_now(&mut self, on: GlowOn, glow: Glow) {
        self.light(on, glow, self.frame);
    }

    /// How long the main loop may wait for the next frame of what moves.
    pub fn motion_wait(&self, now: Instant) -> Option<Duration> {
        self.drawn_motion
            .map(|at| at.saturating_duration_since(now))
    }

    /// How long the main loop waits at `now` when the next tick is
    /// `tick` away: until the next frame of what moves only when it comes
    /// more than [`motion::LATE`] before the tick; else the tick's wake
    /// draws it, a little late, and one frame is drawn, not two.
    pub fn wait(&self, now: Instant, tick: Duration) -> Duration {
        match self.motion_wait(now) {
            Some(frame) if frame + motion::LATE < tick => frame,
            _ => tick,
        }
    }

    /// What changed since the last step that lights a row for a moment:
    /// an agent that came to wait for the person, or a new item in
    /// TO DECIDE, blinks; one whose turn is done, or whose session
    /// started, settles. Only what happens while motion moves lights.
    pub(super) fn watch_glows(&mut self, now: Instant) {
        let rungs = self.rungs();
        if self.moving() != Motion::Off {
            for (id, &rung) in rungs.iter().enumerate() {
                let Some(&was) = self.glow_rungs.get(id) else {
                    continue;
                };
                let glow = match rung {
                    _ if was == rung => continue,
                    Rung::Waiting => Glow::Arrive,
                    Rung::Done => Glow::Settle,
                    _ if was == Rung::Starting => Glow::Settle,
                    _ => continue,
                };
                self.light(GlowOn::Agent(id), glow, now);
            }
            let new = self.glow_items;
            for i in (self.world.open_inbox().into_iter()).filter(|&i| i >= new) {
                self.light(GlowOn::Item(i), Glow::Arrive, now);
            }
        }
        self.glow_rungs = rungs;
        self.glow_items = self.world.inbox.len();
    }

    /// `on` lights with `glow` from `now`, in place of a glow it had.
    /// Its frames count from the first beat at or after `now`, with the
    /// rest of what moves.
    fn light(&mut self, on: GlowOn, glow: Glow, now: Instant) {
        let at = self.epoch + motion::on_beat(now.saturating_duration_since(self.epoch));
        self.glows.retain(|g| g.0 != on);
        self.glows.push((on, glow, at));
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;

    #[test]
    fn the_screen_is_drawn_on_a_change_never_on_a_timer() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        let t0 = Instant::now();
        let s = Duration::from_secs;
        let ms = Duration::from_millis;
        assert!(app.draw_due(t0), "never drawn");
        app.drawn(t0);
        assert!(!app.draw_due(t0 + s(60)), "nothing changed: no timer");
        app.stirred = true;
        assert!(app.draw_due(t0), "a key, a resize");
        app.drawn(t0);
        assert!(!app.stirred, "drawn, stirred no more");
        app.changed = true;
        assert!(app.draw_due(t0), "news from an engine, at once");
        app.drawn(t0);
        assert!(!app.changed && !app.draw_due(t0));
        // The clock: the world's step, a flash coming and going, the
        // seconds a :close waits.
        app.world.tick += 1;
        assert!(app.draw_due(t0), "the world's clock");
        app.drawn(t0);
        let t1 = t0 + s(20);
        app.flash = Some(("x".into(), t1));
        assert!(app.draw_due(t1), "a flash");
        app.drawn(t1);
        assert!(!app.draw_due(t1 + FLASH - ms(1)), "still on show");
        assert!(app.draw_due(t1 + FLASH), "gone");
        app.drawn(t1 + FLASH);
        assert!(!app.draw_due(t1 + FLASH + s(9)));
        let t2 = t1 + s(20);
        app.grace = Some((Undoable::Close(0), t2 + UNDO_GRACE));
        assert!(app.draw_due(t2), "the count starts");
        app.drawn(t2);
        assert!(!app.draw_due(t2 + ms(999)), "10 s still");
        assert!(app.draw_due(t2 + s(1)), "9 s");
        // A step of the demo moves its world.
        app.drawn(t2 + s(1));
        app.step();
        assert!(app.changed && app.draw_due(t2 + s(1)));
    }

    #[test]
    fn another_window_draws_what_changed_once_a_second_and_a_key_at_once() {
        let mut app = App::new(1, 1, 0, 1, Lang::En);
        let t0 = Instant::now();
        let ms = Duration::from_millis;
        // Blurred before the first drawing: drawn.
        app.focus(false, t0);
        assert!(app.draw_due(t0));
        app.drawn(t0);
        assert!(!app.draw_due(t0 + BLURRED_DRAW), "nothing changed");
        app.changed = true;
        assert!(!app.draw_due(t0 + ms(1)));
        assert!(!app.draw_due(t0 + BLURRED_DRAW - ms(1)), "not yet");
        assert!(app.draw_due(t0 + BLURRED_DRAW), "a second on");
        app.drawn(t0 + BLURRED_DRAW);
        app.world.tick += 1;
        assert!(
            !app.draw_due(t0 + BLURRED_DRAW + ms(1)),
            "the clock waits too"
        );
        app.stirred = true;
        assert!(app.draw_due(t0 + BLURRED_DRAW + ms(1)), "a key, at once");
        app.drawn(t0 + BLURRED_DRAW + ms(1));
        app.changed = true;
        let t1 = t0 + BLURRED_DRAW + ms(2);
        assert!(!app.draw_due(t1));
        app.focus(true, t1);
        assert!(app.draw_due(t1), "back: what waited, at once");
    }

    #[test]
    fn a_live_step_draws_only_when_something_came() {
        let mut w = crate::live::testing::team(1);
        // A clock far from now's: the first step moves it.
        w.tick = 1_000;
        let live = crate::live::testing::idle_live(2);
        let mut app = App::with(w, Engine::Live(Box::new(live)), Lang::En);
        let t0 = Instant::now();
        app.step();
        assert!(app.changed, "the clock moved");
        // The file list the first step asked git for comes back first.
        let deadline = t0 + Duration::from_secs(60);
        while !matches!(&app.engine, Engine::Live(l) if crate::live::testing::git_quiet(l)) {
            assert!(Instant::now() < deadline, "git never answered");
            std::thread::sleep(Duration::from_millis(10));
            app.step();
        }
        // A step that leaves the clock where it was says what came.
        let step = |app: &mut App, said: Option<&str>| {
            for _ in 0..3 {
                app.step();
                app.drawn(t0);
                let before = app.world.tick;
                if let (Some(text), Engine::Live(l)) = (said, &app.engine) {
                    let e = imece_agent::Event::Text(text.into());
                    crate::live::testing::hear(l, 1, e);
                }
                app.step();
                if app.world.tick == before {
                    return app.changed;
                }
            }
            panic!("the clock moved on every try");
        };
        assert!(!step(&mut app, None), "nothing came");
        assert!(step(&mut app, Some("hello")), "a session's word");
        // Something waiting to be drawn still is after a quiet step, and
        // after one that brought more.
        app.changed = true;
        app.step();
        assert!(app.changed);
        if let Engine::Live(l) = &app.engine {
            crate::live::testing::hear(l, 1, imece_agent::Event::Text("more".into()));
        }
        app.step();
        assert!(app.changed);
    }

    #[test]
    fn motion_is_off_calm_or_full_by_word_or_round_and_full_is_calm_over_ssh() {
        let (mut app, _) = still(Motion::Off);
        let t = app.lang.text();
        assert_eq!(
            App::new(1, 1, 0, 1, Lang::En).motion,
            Motion::Off,
            "until main says"
        );
        assert_eq!(app.command_note("motion"), "(off)");
        app.command("motion");
        assert_eq!(app.motion, Motion::Calm);
        assert_eq!(flashed(&app).as_deref(), Some(t.motion_calm));
        assert_eq!(app.command_note("motion"), "(calm)");
        app.command("motion");
        assert_eq!(app.motion, Motion::Full);
        assert_eq!(flashed(&app).as_deref(), Some(t.motion_full));
        assert_eq!(app.moving(), Motion::Full);
        app.command("motion");
        assert_eq!(app.motion, Motion::Off);
        assert_eq!(flashed(&app).as_deref(), Some(t.motion_off));
        assert_eq!(app.moving(), Motion::Off);
        app.ssh = true;
        app.command("motion full");
        assert_eq!(app.motion, Motion::Full, "chosen, kept");
        assert_eq!(flashed(&app).as_deref(), Some(t.motion_ssh));
        assert_eq!(app.moving(), Motion::Calm, "runs as calm");
        app.command("motion calm");
        assert_eq!(flashed(&app).as_deref(), Some(t.motion_calm));
        app.command("motion off");
        assert_eq!(flashed(&app).as_deref(), Some(t.motion_off));
        app.command("motion wild");
        assert_eq!(app.motion, Motion::Off, "as it was");
        assert_eq!(
            flashed(&app).as_deref(),
            Some("wild is not off, calm or full")
        );
        app.ssh = false;
        app.motion = Motion::Full;
        app.focus(false, Instant::now());
        assert_eq!(app.moving(), Motion::Off, "nothing moves in another window");
    }

    #[test]
    fn the_mark_comes_a_dot_a_beat_from_when_it_shows_and_whole_when_nothing_moves() {
        let (mut app, _) = still(Motion::Calm);
        let ms = Duration::from_millis;
        let all = ui::brand::DOTS;
        assert_eq!(app.mark_dots(), all, "never shown: whole");
        // First drawn 100 ms in: its dots count from the next beat.
        app.frame_after(ms(100));
        app.mark_shown();
        assert_eq!(app.mark_from.get(), Some(app.after(ms(300))));
        assert!(app.mark_drawn.get());
        for (at, dots) in [
            (100, 0),
            (299, 0),
            (300, 0),
            (599, 0),
            (600, 1),
            (1199, 2),
            (1800, 5),
            (2099, 5),
            (2100, 6),
            (9000, 6),
        ] {
            app.frame_after(ms(at));
            assert_eq!(app.mark_dots(), dots, "at {at} ms");
        }
        // Drawn again in the same drawing, or the next: still coming.
        app.frame_after(ms(900));
        app.mark_shown();
        app.mark_before.set(true);
        app.mark_drawn.set(false);
        app.mark_shown();
        assert_eq!(app.mark_from.get(), Some(app.after(ms(300))), "kept");
        // Gone for a drawing, then back: it comes again.
        app.mark_before.set(false);
        app.mark_drawn.set(false);
        app.frame_after(ms(1200));
        app.mark_shown();
        assert_eq!(
            app.mark_from.get(),
            Some(app.after(ms(1200))),
            "on the beat itself"
        );
        app.frame_after(ms(1800));
        assert_eq!(app.mark_dots(), 2);
        app.motion = Motion::Full;
        assert_eq!(app.mark_dots(), 2, "full comes as calm");
        app.motion = Motion::Off;
        assert_eq!(app.mark_dots(), all, "off: whole");
        app.motion = Motion::Calm;
        app.ssh = true;
        assert_eq!(app.mark_dots(), all, "over SSH: whole");
        app.ssh = false;
        app.focus(false, Instant::now());
        assert_eq!(app.mark_dots(), all, "another window: whole");
        app.focus(true, Instant::now());
        assert_eq!(app.mark_dots(), 2);
        app.on_key(KeyEvent::from(KeyCode::Char('j')));
        assert_eq!(app.mark_dots(), all, "a key: whole");
        assert_eq!(app.mark_from.get(), None);
    }

    #[test]
    fn the_mark_moves_only_where_it_was_drawn_and_until_it_is_whole() {
        let (app, _) = still(Motion::Calm);
        let ms = Duration::from_millis;
        app.mark_from.set(Some(app.after(ms(300))));
        assert_eq!(app.next_motion(app.after(ms(0))), None, "not drawn");
        app.mark_drawn.set(true);
        assert_eq!(
            app.moving_parts(app.after(ms(0))),
            [(Moves::Mark, motion::BEAT)]
        );
        assert_eq!(app.next_motion(app.after(ms(0))), Some(app.after(ms(300))));
        assert_eq!(
            app.next_motion(app.after(ms(1800))),
            Some(app.after(ms(2100))),
            "the last dot"
        );
        assert_eq!(
            app.next_motion(app.after(ms(2099))),
            Some(app.after(ms(2100)))
        );
        assert_eq!(app.next_motion(app.after(ms(2100))), None, "whole");
        app.mark_from.set(None);
        assert_eq!(
            app.next_motion(app.after(ms(0))),
            None,
            "a key made it whole"
        );
        app.mark_from.set(Some(app.after(ms(300))));
        let mut app = app;
        app.ssh = true;
        assert_eq!(app.next_motion(app.after(ms(0))), None, "SSH");
    }

    #[test]
    fn each_thing_that_moves_is_drawn_as_it_is_at_the_frame() {
        let (mut app, w) = still(Motion::Off);
        let ms = Duration::from_millis;
        let e = app.epoch;
        let at = |app: &mut App, n: u64| app.frame = e + ms(n);
        at(&mut app, 700);
        app.world.agents[w].status = Status::Working;
        assert!(!app.breath());
        assert_eq!(
            (app.dots(), app.spin(), app.head_spin()),
            (None, None, None)
        );
        app.motion = Motion::Calm;
        at(&mut app, 599);
        assert!(!app.breath());
        assert_eq!(app.dots(), Some("   "));
        assert_eq!(app.spin(), Some(motion::SPIN[1]));
        assert_eq!(app.head_spin(), None, "calm has none");
        at(&mut app, 600);
        assert!(app.breath(), "the second tone");
        assert_eq!(app.dots(), Some(".  "));
        assert_eq!(app.spin(), Some(motion::SPIN[2]));
        at(&mut app, 1200);
        assert!(!app.breath());
        assert_eq!(app.dots(), Some(".. "));
        at(&mut app, 1800);
        assert!(app.breath());
        assert_eq!(app.dots(), Some("..."));
        at(&mut app, 2400);
        assert_eq!(app.dots(), Some("   "), "round again");
        assert_eq!(app.spin(), Some(motion::SPIN[8]));
        at(&mut app, 3300);
        assert_eq!(app.spin(), Some(motion::SPIN[1]), "eleven beats: round");
        app.motion = Motion::Full;
        at(&mut app, 250);
        assert_eq!(
            app.spin(),
            Some(motion::SPIN[2]),
            "a tenth of a second each"
        );
        assert_eq!(app.head_spin(), Some(motion::SPIN[2]));
        at(&mut app, 1150);
        assert_eq!(app.head_spin(), Some(motion::SPIN[1]));
        app.world.agents[w].status = Status::Idle;
        assert_eq!(app.head_spin(), None, "no one mid-turn");
        app.world.agents[w].status = Status::Working;
        app.ssh = true;
        assert_eq!(app.head_spin(), None, "calm over SSH");
        assert_eq!(app.spin(), Some(motion::SPIN[3]));
        app.ssh = false;
        app.focus(false, Instant::now());
        assert!(!app.breath());
        assert_eq!(
            (app.dots(), app.spin(), app.head_spin()),
            (None, None, None)
        );
    }

    #[test]
    fn what_comes_to_wait_blinks_and_what_is_done_or_started_settles() {
        let (mut app, w) = still(Motion::Calm);
        let ms = Duration::from_millis;
        let e = app.epoch;
        let me = GlowOn::Agent(w);
        app.watch_glows(e);
        assert!(app.glows.is_empty(), "the first look lights nothing");
        // A question: its row and its item blink from the next beat.
        app.world.inbox.push(InboxItem {
            kind: InboxKind::Question,
            agent: w,
            detail: "which?".into(),
            researcher: None,
            open: true,
            tick: 0,
            card: None,
        });
        app.watch_glows(e + ms(1000));
        assert_eq!(
            app.glows,
            [
                (me, Glow::Arrive, e + ms(1200)),
                (GlowOn::Item(0), Glow::Arrive, e + ms(1200)),
            ]
        );
        let lit = |app: &mut App, n: u64, on: GlowOn| {
            app.frame = e + ms(n);
            app.lit(on)
        };
        assert_eq!(lit(&mut app, 1000, me), Some(Lit::Reversed), "at once");
        assert_eq!(lit(&mut app, 1499, me), Some(Lit::Reversed));
        assert_eq!(lit(&mut app, 1500, me), None);
        assert_eq!(lit(&mut app, 1800, GlowOn::Item(0)), Some(Lit::Reversed));
        assert_eq!(lit(&mut app, 3000, me), None, "over");
        assert_eq!(lit(&mut app, 1000, GlowOn::Item(1)), None);
        app.watch_glows(e + ms(1100));
        assert_eq!(app.glows.len(), 2, "nothing new, nothing lit");
        // Answered and done: it settles; an item seen once is not new.
        app.world.inbox[0].open = false;
        app.unseen.insert(w);
        app.watch_glows(e + ms(2000));
        assert_eq!(app.glows[1], (me, Glow::Settle, e + ms(2100)));
        assert_eq!(lit(&mut app, 2400, me), Some(Lit::Settle(1)));
        // Seen, then starting, then started: it settles again.
        app.unseen.clear();
        app.world.agents[w].status = Status::Working;
        app.world.agents[w].starting = true;
        app.watch_glows(e + ms(4000));
        assert_eq!(app.glows.len(), 2, "idle to starting lights nothing");
        assert_eq!(app.glows[1], (me, Glow::Settle, e + ms(2100)), "as it was");
        app.world.agents[w].starting = false;
        app.watch_glows(e + ms(4200));
        assert_eq!(
            app.glows[1],
            (me, Glow::Settle, e + ms(4200)),
            "on the beat"
        );
        assert_eq!(app.glows.len(), 2, "in place of its last");
        // Working to idle, nothing to say.
        app.world.agents[w].status = Status::Idle;
        app.watch_glows(e + ms(5000));
        assert_eq!(app.glows.len(), 2);
        assert_eq!(app.glows[1], (me, Glow::Settle, e + ms(4200)), "as it was");
        // Motion off, or another window: nothing lights, and what changed
        // meanwhile does not light later.
        app.motion = Motion::Off;
        app.unseen.insert(w);
        app.world.inbox.push(app.world.inbox[0].clone());
        app.world.inbox[1].open = true;
        app.watch_glows(e + ms(6000));
        assert_eq!(app.glows.len(), 2);
        assert_eq!(lit(&mut app, 4300, me), None, "off: none shown");
        app.motion = Motion::Calm;
        app.watch_glows(e + ms(7000));
        assert_eq!(app.glows.len(), 2, "already seen");
        // An app that starts with items waiting lights none of them.
        let (mut world, sim) = sim::world(1, 1, 0, 1, Lang::En);
        world.inbox.push(app.world.inbox[1].clone());
        let mut fresh = App::with(world, Engine::Sim(sim), Lang::En);
        fresh.motion = Motion::Calm;
        fresh.watch_glows(e);
        assert!(fresh.glows.is_empty());
    }

    #[test]
    fn the_next_frame_is_when_what_is_shown_moves_and_never_when_nothing_does() {
        let (mut app, w) = still(Motion::Calm);
        let ms = Duration::from_millis;
        let e = app.epoch;
        let now = e + ms(100);
        assert_eq!(app.next_motion(now), None, "nothing moves");
        app.world.agents[w].status = Status::Working;
        assert_eq!(app.next_motion(now), Some(e + ms(600)), "it breathes");
        assert_eq!(app.next_motion(e + ms(600)), Some(e + ms(1200)));
        app.window = Some(Window::Board);
        app.world.cards.clear();
        assert_eq!(app.next_motion(now), None, "not on the screen");
        // On the board, the owner of a card taken breathes.
        app.world.cards.push(Card {
            id: 0,
            project: 0,
            title: "t".into(),
            owner: Some(w),
            column: Column::Working,
            story: Vec::new(),
            symbols: Vec::new(),
            paths: Vec::new(),
            started: 0,
            finished: 0,
            attempt: 1,
            breaker: Default::default(),
            after: Vec::new(),
        });
        assert_eq!(app.next_motion(now), Some(e + ms(600)), "its card's owner");
        app.world.cards[0].column = Column::Done;
        assert_eq!(
            app.next_motion(now),
            Some(e + ms(600)),
            "done, still at work"
        );
        app.world.cards[0].column = Column::Waiting;
        assert_eq!(app.next_motion(now), None, "no owner shown");
        app.world.cards[0].column = Column::Working;
        app.world.cards[0].owner = None;
        assert_eq!(app.next_motion(now), None, "nobody's");
        app.world.cards[0].owner = Some(w);
        app.world.agents[w].gone = true;
        assert_eq!(app.next_motion(now), None, "its owner gone");
        app.world.agents[w].gone = false;
        app.world.projects[0].archived = Some(Archived {
            at: 0,
            title: "t".into(),
        });
        assert_eq!(app.next_motion(now), None, "in its archive");
        app.world.projects[0].archived = None;
        app.zoom = Zoom::Focus(w);
        assert_eq!(
            app.next_motion(now),
            Some(e + ms(600)),
            "the board's window over a zoom"
        );
        app.zoom = Zoom::None;
        app.window = Some(Window::Code);
        assert_eq!(app.next_motion(now), None, "the code map over the team");
        app.window = None;
        app.zoom = Zoom::Focus(w);
        assert_eq!(app.next_motion(now), None);
        app.zoom = Zoom::None;
        app.world.agents[w].gone = true;
        assert_eq!(app.next_motion(now), None, "gone");
        app.world.agents[w].gone = false;
        app.world.projects[0].closed = true;
        assert_eq!(app.next_motion(now), None, "put away");
        app.world.projects[0].closed = false;
        app.motion = Motion::Off;
        assert_eq!(app.next_motion(now), None, "off");
        app.motion = Motion::Calm;
        // A leader working does not breathe; starting counts its dots.
        app.world.agents[w].status = Status::Idle;
        let lead = app.world.projects[0].leader;
        app.world.agents[lead].status = Status::Working;
        assert_eq!(app.next_motion(now), None);
        app.world.agents[lead].status = Status::Idle;
        app.world.agents[w].starting = true;
        assert_eq!(app.next_motion(now), None, "starting, not working");
        app.world.agents[w].status = Status::Working;
        assert_eq!(app.rungs()[w], Rung::Starting);
        assert_eq!(
            app.next_motion(now),
            None,
            "starting breathes nothing on Home"
        );
        app.world.agents[w].starting = false;
        app.world.agents[w].status = Status::Idle;
        // Waiting for an answer: the bottom line spins when nothing else
        // is said there.
        app.asking = Some("asking".into());
        assert_eq!(app.next_motion(now), Some(e + ms(300)));
        app.flash = Some(("x".into(), now));
        assert_eq!(app.next_motion(now), None, "a flash says something");
        assert_eq!(
            app.next_motion(now + FLASH),
            Some(e + ms(3300)),
            "gone at its end"
        );
        app.flash = None;
        app.typing = true;
        assert_eq!(app.next_motion(now), None, "writing");
        app.typing = false;
        app.grace = Some((Undoable::Close(0), now + UNDO_GRACE));
        assert_eq!(app.next_motion(now), None, "a countdown");
        app.grace = None;
        app.asking = None;
        // The first-run window checking Claude Code spins.
        app.welcome = true;
        app.seen = Seen::Checking;
        assert_eq!(app.next_motion(now), Some(e + ms(300)));
        app.seen = Seen::Missing;
        assert_eq!(app.next_motion(now), None, "checked");
        app.seen = Seen::Checking;
        app.welcome = false;
        assert_eq!(app.next_motion(now), None, "not shown");
        // Full: the header spins while anyone works, on any screen.
        app.motion = Motion::Full;
        app.window = Some(Window::Board);
        assert_eq!(app.next_motion(now), None, "no one works");
        app.world.agents[lead].status = Status::Working;
        assert_eq!(app.next_motion(now), Some(e + ms(200)));
        app.window = None;
        app.world.agents[lead].status = Status::Idle;
        app.motion = Motion::Calm;
        // A glow: from its beat to its end, then nothing.
        app.light(GlowOn::Agent(w), Glow::Settle, now);
        assert_eq!(app.glows[0].2, e + ms(300));
        assert_eq!(
            app.next_motion(now),
            Some(e + ms(600)),
            "the beat after its own"
        );
        assert_eq!(app.next_motion(e + ms(700)), Some(e + ms(900)));
        assert_eq!(app.next_motion(e + ms(1100)), Some(e + ms(1200)), "its end");
        assert_eq!(app.next_motion(e + ms(1200)), None);
        // The soonest of all.
        app.world.agents[w].status = Status::Working;
        assert_eq!(app.next_motion(e + ms(700)), Some(e + ms(900)));
        assert_eq!(app.next_motion(e + ms(1000)), Some(e + ms(1200)));
        app.viewing = Some(Box::new(App::new(1, 1, 0, 1, Lang::En)));
        assert_eq!(app.next_motion(now), None, "an archive's view over it");
        app.viewing = None;
        app.focus(false, now);
        assert_eq!(app.next_motion(now), None, "another window");
    }

    #[test]
    fn a_step_looks_for_what_lights() {
        let (mut app, _) = still(Motion::Calm);
        assert!(app.glow_rungs.is_empty());
        app.step();
        assert_eq!(
            app.glow_rungs,
            app.rungs(),
            "seen, to tell what is new next"
        );
        assert_eq!(app.glow_items, app.world.inbox.len());
    }

    #[test]
    fn what_moves_is_drawn_on_its_next_frame_and_never_in_another_window() {
        let (mut app, w) = still(Motion::Calm);
        let ms = Duration::from_millis;
        let e = app.epoch;
        app.world.agents[w].status = Status::Working;
        let t0 = e + ms(100);
        assert!(app.draw_due(t0));
        app.drawn(t0);
        assert_eq!(app.motion_wait(t0), Some(ms(500)));
        assert_eq!(app.motion_wait(t0 + ms(700)), Some(ms(0)), "late: at once");
        assert!(!app.draw_due(t0 + ms(499)));
        assert!(app.draw_due(t0 + ms(500)), "the next tone");
        app.drawn(t0 + ms(500));
        assert_eq!(app.motion_wait(t0 + ms(500)), Some(ms(600)));
        // A glow is kept to its end, then let go.
        app.light(GlowOn::Agent(w), Glow::Arrive, t0 + ms(500));
        app.drawn(e + ms(2399));
        assert_eq!(app.glows.len(), 1);
        app.drawn(e + ms(2400));
        assert!(app.glows.is_empty(), "over");
        // Not yet begun: kept.
        app.light(GlowOn::Agent(w), Glow::Arrive, e + ms(2500));
        app.drawn(e + ms(2500));
        assert_eq!(app.glows.len(), 1);
        // In another window nothing waits for a frame.
        app.focus(false, e + ms(2600));
        assert_eq!(app.motion_wait(e + ms(2600)), None);
        assert!(!app.draw_due(e + ms(4000)));
        app.drawn(e + ms(2600));
        assert_eq!(app.motion_wait(e + ms(2600)), None);
        app.motion = Motion::Off;
        app.focus(true, e + ms(2700));
        app.drawn(e + ms(2700));
        assert_eq!(app.motion_wait(e + ms(2700)), None, "off");
    }

    #[test]
    fn a_frame_is_patched_only_when_the_breath_alone_moved() {
        let (mut app, w) = still(Motion::Calm);
        let ms = Duration::from_millis;
        app.world.agents[w].status = Status::Working;
        let glyph = ui::Glyph {
            x: 5,
            y: 4,
            width: 2,
        };
        *app.breathing.borrow_mut() = Some(vec![glyph]);
        let t0 = app.after(ms(100));
        app.drawn(t0);
        let t1 = app.after(ms(600));
        assert!(app.patchable());
        assert!(app.patch_due(t1), "the breath alone");
        app.stirred = true;
        assert!(!app.patch_due(t1), "a key");
        app.stirred = false;
        app.changed = true;
        assert!(!app.patch_due(t1), "the world");
        app.changed = false;
        app.world.tick += 1;
        assert!(!app.patch_due(t1), "the clock");
        app.world.tick -= 1;
        app.light(GlowOn::Agent(w), Glow::Arrive, t0);
        assert!(!app.patch_due(t1), "a glow");
        app.glows.clear();
        app.world.agents[w].starting = true;
        assert!(!app.patch_due(t1), "dots");
        app.world.agents[w].starting = false;
        *app.breathing.borrow_mut() = Some(Vec::new());
        assert!(!app.patchable());
        assert!(!app.patch_due(t1), "nothing known breathes");
        *app.breathing.borrow_mut() = None;
        assert!(!app.patchable());
        assert!(!app.patch_due(t1), "not known");
        *app.breathing.borrow_mut() = Some(vec![glyph]);
        app.motion = Motion::Off;
        assert!(!app.patch_due(t1), "off");
        app.motion = Motion::Calm;
        assert!(app.patch_due(t1), "again");
    }

    #[test]
    fn a_frame_near_the_tick_is_drawn_with_it() {
        let (mut app, w) = still(Motion::Calm);
        let ms = Duration::from_millis;
        let t0 = app.epoch + ms(100);
        app.world.agents[w].status = Status::Working;
        app.drawn(t0);
        // The next tone 500 ms on.
        assert_eq!(app.wait(t0, ms(601)), ms(500), "far from the tick: its own");
        assert_eq!(app.wait(t0, ms(600)), ms(600), "100 ms before: the tick's");
        assert_eq!(app.wait(t0, ms(550)), ms(550));
        assert_eq!(app.wait(t0, ms(400)), ms(400), "after the tick");
        app.motion = Motion::Off;
        app.drawn(t0);
        assert_eq!(app.wait(t0, ms(700)), ms(700), "nothing moves");
    }

    #[test]
    fn a_commit_message_asked_for_spins_until_it_comes() {
        let (mut app, _) = still(Motion::Calm);
        let t = app.lang.text();
        app.command("commit");
        let asked = app.asking().map(str::to_string);
        assert!(asked.is_some(), "{:?}", flashed(&app));
        assert_eq!(flashed(&app), asked, "said first, then spun");
        assert!(asked.unwrap().contains(&fill(t.suggesting, &["", ""])[..6]));
        app.step();
        assert!(app.asking().is_some(), "not yet");
        app.step();
        assert_eq!(app.asking(), None, "it came");
        // A refusal is only said.
        app.world.budget = app.world.spent();
        app.command("commit");
        assert_eq!(app.asking(), None);
    }
}
