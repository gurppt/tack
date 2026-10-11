use super::*;
#[test]
fn exact_sequence_every_frame_hidden_until_full_wait_and_fixed_landing() {
    let now = Instant::now();
    let mut g = Game::new(now, [410., 300.], 3);
    g.at = [400., 300.];
    g.saved = [410., 300.];
    g.enter(State::Attack, now);
    let mut t = now;
    for frame in 0..15 {
        assert_eq!(g.frame, frame);
        assert_eq!(g.hidden, frame >= 10);
        if frame == 9 {
            assert_eq!(g.at, [400., 300.]);
            assert_eq!(g.advances, 0);
        }
        if frame >= 10 {
            assert_eq!(g.at, [410., 300.]);
            assert_eq!(g.advances, 1);
        }
        g.pointer([700., 500.]);
        t += ATTACK_FRAME;
        assert!(g.tick(t, [800., 600.]));
    }
    assert_eq!(g.state, State::Munch);
    let start = t;
    let landing = g.at;
    let mut munch_frame = 0;
    while t - start < MUNCH {
        assert!(g.hidden);
        assert_eq!(g.at, landing);
        assert_eq!(g.frame, munch_frame % 2);
        munch_frame += 1;
        t = g.deadline().unwrap_or(t + MUNCH_FRAME);
        g.tick(t, [800., 600.]);
    }
    assert_eq!(g.state, State::Explosion);
    for frame in 0..12 {
        assert_eq!(g.frame, frame);
        assert!(g.hidden);
        assert_eq!(g.at, landing);
        t += EXPLOSION_FRAME;
        g.tick(t, [800., 600.]);
    }
    assert_eq!(g.state, State::PostExplosionWait);
    assert!(g.hidden);
    assert!(!g.tick(t + Duration::from_millis(999), [800., 600.]));
    assert!(g.hidden);
    assert!(g.tick(t + AFTER, [800., 600.]));
    assert_eq!(g.state, State::Finished);
    assert!(!g.hidden);
    assert!(g.deadline().is_none());
}
#[test]
fn late_clock_never_skips_attack_frame_eleven_or_explosion_frames() {
    let mut t = Instant::now();
    let mut g = Game::new(t, [100., 100.], 0);
    g.at = [90., 100.];
    g.saved = [100., 100.];
    g.enter(State::Attack, t);
    for frame in 1..15 {
        t += Duration::from_secs(20);
        g.tick(t, [800., 600.]);
        assert_eq!(g.frame, frame);
        assert_eq!(g.hidden, frame >= 10);
    }
    assert_eq!(g.advances, 1);
}
#[test]
fn anchored_jitter_accepted_but_accumulated_escape_cancels_dwell() {
    let now = Instant::now();
    let mut g = Game::new(now, [400., 300.], 0);
    g.at = g.dock_target();
    g.reference = g.target;
    g.dwell = Some(now);
    g.enter(State::Idle, now);
    for n in 1..=16 {
        g.pointer([400. + if n % 2 == 0 { 3. } else { -3. }, 300.]);
        g.tick(now + Duration::from_millis(n * 250), [800., 600.]);
    }
    assert_eq!(g.state, State::Attack);
    g.enter(State::Idle, now);
    g.reference = [400., 300.];
    g.dwell = Some(now);
    g.pointer([402., 300.]);
    g.tick(now + STEP, [800., 600.]);
    assert_eq!(g.state, State::Idle);
    g.pointer([404., 300.]);
    g.tick(now + STEP * 2, [800., 600.]);
    assert_eq!(g.state, State::Run);
    assert!(g.dwell.is_none());
}
#[test]
fn bounded_spawn_spring_converges_and_smoke_stays_put() {
    let now = Instant::now();
    for seed in 0..4 {
        let mut g = Game::new(now, [400., 300.], seed);
        assert!(!g.tick(now + SPAWN - STEP, [800., 600.]));
        g.tick(now + SPAWN, [800., 600.]);
        assert!(g.at[0] < 0. || g.at[0] > 800. || g.at[1] < 0. || g.at[1] > 600.);
        let mut previous = None;
        let mut saw_idle = false;
        for n in 1..700 {
            let t = now + SPAWN + STEP * n;
            g.tick(t, [800., 600.]);
            assert!(g.velocity[0].hypot(g.velocity[1]) <= MAX_SPEED + 0.001);
            assert!(g.puffs.len() <= SMOKE_LIMIT);
            if let Some((at, b, flip)) = previous
                && let Some(p) = g.puffs.front().filter(|p| p.at == at)
            {
                assert_eq!(p.b, b);
                assert_eq!(p.flip, flip);
            }
            previous = g.puffs.front().map(|p| (p.at, p.b, p.flip));
            saw_idle |= g.state == State::Idle;
            if g.state == State::Attack {
                break;
            }
        }
        assert!(saw_idle);
        assert_eq!(g.state, State::Attack);
    }
}
#[test]
fn art_counts_dimensions_bytes_orientation_and_overlay_bound() -> Result<(), tack_assets::AssetError>
{
    use std::sync::atomic::AtomicBool;
    let art = artwork::Artwork::load(&artwork::root(), &AtomicBool::new(false))?;
    assert!(art.mask_bytes() < 256 * 1024);
    assert_eq!(art.cheese.len(), 1024);
    let mut g = Game::new(Instant::now(), [400., 300.], 0);
    g.enter(State::Run, Instant::now());
    let c = tack_core::Camera::new([800, 600]);
    let mut overlay = crate::image_gizmo::ImageGizmo::default();
    art.draw(&g, &mut overlay, &c);
    let original = overlay
        .quads
        .iter()
        .filter_map(|q| q.bitmap)
        .collect::<Vec<_>>();
    g.flip = true;
    overlay.quads.clear();
    art.draw(&g, &mut overlay, &c);
    assert_ne!(
        original,
        overlay
            .quads
            .iter()
            .filter_map(|q| q.bitmap)
            .collect::<Vec<_>>()
    );
    Ok(())
}
#[test]
fn cancellation_disposes_every_stage_and_easter_deadline() {
    let now = Instant::now();
    for state in [
        State::SpawnDelay,
        State::Run,
        State::Idle,
        State::Attack,
        State::Munch,
        State::Explosion,
        State::PostExplosionWait,
    ] {
        let mut mouse = crate::mouse_tool::MouseTool::default();
        mouse.activate(true, now);
        let mut game = Game::new(now, [400., 300.], 7);
        game.enter(state, now);
        game.hidden = matches!(
            state,
            State::Attack | State::Munch | State::Explosion | State::PostExplosionWait
        );
        mouse.game = Some(Box::new(game));
        let ticket = mouse.ticket;
        mouse.activate(false, now + STEP);
        assert!(mouse.game.is_none());
        assert!(mouse.artwork.is_none());
        assert!(mouse.deadline().is_none());
        assert_ne!(mouse.ticket, ticket);
        assert!(!mouse.activate(false, now + STEP * 2));
    }
}
#[test]
fn repeated_activation_is_one_instance_and_easter_clock_survives_animation() {
    let now = Instant::now();
    let mut mouse = crate::mouse_tool::MouseTool::default();
    mouse.activate(true, now);
    mouse.game = Some(Box::new(Game::new(now, [400., 300.], 1)));
    let ticket = mouse.ticket;
    assert!(!mouse.activate(true, now + SPAWN));
    assert_eq!(mouse.ticket, ticket);
    assert!(mouse.game.is_some());
    assert!(!mouse.settle(now + Duration::from_secs(599)).1);
    assert!(mouse.settle(now + Duration::from_secs(600)).1);
    assert!(!mouse.settle(now + Duration::from_secs(1200)).1);
    assert_eq!(mouse.spawns, 1);
    mouse.activate(false, now + Duration::from_secs(1200));
    assert!(mouse.deadline().is_none());
}
#[test]
fn fixed_puffs_alternate_with_emission_orientation_and_expire() {
    let now = Instant::now();
    let mut g = Game::new(now, [10000., 300.], 1);
    g.enter(State::Run, now);
    g.flip = true;
    g.velocity = [MAX_SPEED, 0.];
    for n in 0..SMOKE_LIMIT + 9 {
        g.at = [n as f64 * 40. + 40., 300.];
        g.smoke([n as f64 * 40., 300.], now);
    }
    assert_eq!(g.puffs.len(), SMOKE_LIMIT);
    for i in 1..g.puffs.len() {
        assert_ne!(g.puffs[i].b, g.puffs[i - 1].b);
        assert!(!g.puffs[i].flip);
    }
    let first = g.puffs[0].at;
    g.at = [400., 300.];
    assert_eq!(g.puffs[0].at, first);
    g.reference = g.at;
    g.target = g.at;
    g.at = g.dock_target();
    g.dwell = Some(now);
    g.enter(State::Idle, now);
    for n in 1..=5 {
        g.tick(now + SMOKE_FRAME * n, [800., 600.]);
    }
    assert!(g.puffs.is_empty());
}
#[test]
fn native_presentation_gate_does_not_advance_unrendered_attack_frame() {
    let now = Instant::now();
    let mut g = Game::new(now, [400., 300.], 1);
    g.enter(State::Attack, now);
    g.require_presentation();
    g.tick(now + ATTACK_FRAME, [800., 600.]);
    assert_eq!(g.frame, 1);
    assert!(!g.tick(now + ATTACK_FRAME * 2, [800., 600.]));
    assert_eq!(g.frame, 1);
    g.presented();
    g.tick(now + ATTACK_FRAME * 3, [800., 600.]);
    assert_eq!(g.frame, 2);
}
#[test]
fn explicit_tool_switch_cannot_leave_temporary_mouse_override_active()
-> Result<(), tack_assets::AssetError> {
    use crate::actions::{Action, ActionEvent, ActionPhase, HoldToken, Tool};
    let mut input = crate::image_input::ImageInput::new()?;
    let mut editor = tack_core::DocumentEditor::new(
        tack_core::Document::new(tack_storage::new_document_id()?, Default::default()),
        32,
    );
    let mut camera = tack_core::Camera::new([800, 600]);
    input.dispatch(
        ActionEvent {
            action: Action::TemporaryTool(Tool::Mouse),
            phase: ActionPhase::Begin(HoldToken(1)),
        },
        &mut editor,
        &mut camera,
    )?;
    assert_eq!(input.active_tool(), Tool::Mouse);
    input.dispatch(
        ActionEvent {
            action: Action::SelectTool(Tool::Pointer),
            phase: ActionPhase::Invoke,
        },
        &mut editor,
        &mut camera,
    )?;
    assert_eq!(input.active_tool(), Tool::Pointer);
    assert_eq!(editor.generation(), 0);
    Ok(())
}
#[test]
fn all_authored_sprite_cycles_use_twelve_frames_per_second() {
    assert_eq!(ATTACK_FRAME, FRAME);
    assert_eq!(MUNCH_FRAME, FRAME);
    assert_eq!(EXPLOSION_FRAME, FRAME);
    assert_eq!(SMOKE_FRAME, FRAME);
    assert!((FRAME.as_secs_f64() * 12. - 1.).abs() < 0.000001);
    let now = Instant::now();
    let mut g = Game::new(now, [400., 300.], 0);
    g.enter(State::Idle, now);
    g.reference = g.target;
    g.at = g.dock_target();
    g.dwell = Some(now);
    for n in 1..=12 {
        g.tick(now + FRAME * n, [800., 600.]);
        assert_eq!(g.frame, n as usize % 3);
    }
}
#[test]
fn lateral_dock_accounts_for_scaled_canvases_hotspot_and_edges() {
    let now = Instant::now();
    for point in [[400., 300.], [2., 300.], [798., 300.]] {
        for right in [false, true] {
            let mut g = Game::new(now, point, 0);
            g.dock_right = right;
            g.fit_side(800.);
            let dock = g.dock_target();
            let half = SPRITE_WIDTH / 2.;
            let gap = if g.dock_right {
                (dock[0] - half) - (point[0] + SPRITE_WIDTH - CHEESE_HOTSPOT)
            } else {
                (point[0] - CHEESE_HOTSPOT) - (dock[0] + half)
            };
            assert_eq!(gap, 2.);
            assert_eq!(dock[1] - half, point[1] + half - CHEESE_HOTSPOT);
            assert!(dock[0] - half >= 0. && dock[0] + half <= 800.);
        }
    }
}
#[test]
fn approach_from_above_below_left_and_right_parks_laterally_facing_cheese() {
    let now = Instant::now();
    for seed in 0..4 {
        let mut g = Game::new(now, [400., 300.], seed);
        g.tick(now + SPAWN, [800., 600.]);
        for n in 1..700 {
            g.tick(now + SPAWN + STEP * n, [800., 600.]);
            if g.state == State::Idle {
                break;
            }
        }
        assert_eq!(g.state, State::Idle);
        assert_eq!(g.at, g.dock_target());
        assert_eq!(g.at[1], 318.);
        assert_eq!(g.flip, !g.dock_right);
        assert_eq!((g.at[0] - 402.).abs(), 34.);
    }
}
#[test]
fn footprints_are_black_and_behind_smoke_and_character() -> Result<(), tack_assets::AssetError> {
    use std::sync::atomic::AtomicBool;
    let now = Instant::now();
    let mut mouse = crate::mouse_tool::MouseTool::default();
    mouse.load();
    mouse.activate(true, now);
    mouse.motion([200., 200.], now);
    mouse.motion([256., 200.], now);
    assert_eq!(mouse.effect_count(), 2);
    let mut game = Game::new(now, [240., 200.], 0);
    game.enter(State::Run, now);
    game.puffs.push_back(Puff {
        at: [230., 200.],
        b: false,
        flip: false,
        frame: 0,
        next: now + FRAME,
    });
    mouse.game = Some(Box::new(game));
    mouse.artwork = Some(Box::new(artwork::Artwork::load(
        &artwork::root(),
        &AtomicBool::new(false),
    )?));
    let c = tack_core::Camera::new([800, 600]);
    let mut full = crate::image_gizmo::ImageGizmo::default();
    mouse.draw(&mut full, &c);
    let mut foreground = crate::image_gizmo::ImageGizmo::default();
    if let (Some(art), Some(game)) = (&mouse.artwork, &mouse.game) {
        art.draw(game, &mut foreground, &c);
    }
    assert_eq!(full.quads.len(), 2 + foreground.quads.len());
    assert!(full.quads[..2].iter().all(|q| q.color == [0., 0., 0., 1.]));
    for (actual, expected) in full.quads[2..].iter().zip(&foreground.quads) {
        assert_eq!(actual.color, expected.color);
        assert_eq!(actual.bitmap, expected.bitmap);
        assert_eq!(actual.points, expected.points);
    }
    // The first foreground mask is smoke grey, before the coloured character.
    assert!(foreground.quads[0].color[3] < 1.);
    assert!(foreground.quads.iter().any(|q| q.color[0] != q.color[1]));
    Ok(())
}

#[test]
fn attack_lands_horizontally_on_frozen_cheese_from_either_side() {
    let now = Instant::now();
    for right in [false, true] {
        let mut g = Game::new(now, [400., 300.], 0);
        g.dock_right = right;
        g.at = g.dock_target();
        let parked = g.at;
        g.saved = [402., 300.];
        g.enter(State::Attack, now);
        for frame in 1..15 {
            g.pointer([720., 500.]);
            g.tick(now + FRAME * frame, [800., 600.]);
            assert_eq!(g.at[1], parked[1]);
            assert_eq!(g.at[0], if frame < 10 { parked[0] } else { 402. });
            assert_eq!(g.advances, u32::from(frame >= 10));
        }
        assert_eq!((parked[0] - g.at[0]).abs(), 34.);
    }
}
