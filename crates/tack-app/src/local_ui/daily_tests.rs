use super::*;
use tack_core::{BookmarkId, CameraBookmark};
type R = Result<(), tack_assets::AssetError>;
#[test]
fn bounded_bookmark_panels_fit_small_window_and_shared_controls_refuse_edits() -> R {
    let profile = Preferences::defaults()?;
    let keymap = profile.keymap()?;
    let bookmarks = (1..=64)
        .map(|i| {
            Ok(CameraBookmark::new(
                BookmarkId::new(i)?,
                format!("View {i}"),
                [0.; 2],
                1.,
            )?)
        })
        .collect::<Result<Vec<_>, tack_assets::AssetError>>()?;
    for shared in [false, true] {
        for scale in [1., 2.] {
            let mut ui = LocalUi::daily(
                Panel::Bookmarks,
                DailyPanel {
                    bookmarks: bookmarks.clone(),
                    shared,
                    ..Default::default()
                },
            );
            let mut camera = Camera::new([800, 600]);
            camera.set_ui_scale(scale);
            let mut gizmo = ImageGizmo::default();
            ui.selected = 63;
            ui.draw(&mut gizmo, &camera, &keymap, &profile);
            assert!(ui.visible <= 12 && ui.first + ui.visible >= 64);
            for hit in &ui.hits {
                assert!(
                    hit.rect[0] >= 0. && hit.rect[2] * scale <= 800. && hit.rect[3] * scale <= 600.
                );
            }
            assert!(matches!(
                ui.daily_activate(),
                Some(UiResult::JumpBookmark(_))
            ));
            assert_eq!(ui.daily_command(true).is_some(), !shared);
            assert_eq!(ui.daily_command(false).is_some(), !shared);
        }
    }
    Ok(())
}
#[test]
fn native_form_buttons_confirm_and_cancel_without_hidden_action() -> R {
    let mut profile = Preferences::defaults()?;
    let mut keymap = profile.keymap()?;
    let mut ui = LocalUi::daily(
        Panel::BookmarkName,
        DailyPanel {
            text: "Overview".into(),
            ..Default::default()
        },
    );
    assert!(ui.daily_activate().is_none());
    assert!(matches!(
        ui.command(Command::ConfirmDaily, &mut keymap, &mut profile),
        Some(UiResult::SaveBookmark(None, _))
    ));
    ui.selected = 2;
    assert!(matches!(ui.daily_activate(), Some(UiResult::Dismiss)));
    Ok(())
}
#[test]
fn empty_bookmark_list_exposes_creation_and_shared_form_has_no_edit_path() -> R {
    let mut profile = Preferences::defaults()?;
    let mut keymap = profile.keymap()?;
    for size in [[800, 600], [1024, 768]] {
        for scale in [1., 2.] {
            for shared in [false, true] {
                let mut ui = LocalUi::daily(
                    Panel::Bookmarks,
                    DailyPanel {
                        shared,
                        ..Default::default()
                    },
                );
                let mut c = Camera::new(size);
                c.set_ui_scale(scale);
                let mut g = ImageGizmo::default();
                ui.draw(&mut g, &c, &keymap, &profile);
                let add = ui
                    .hits
                    .iter()
                    .find(|h| h.command == Command::AddBookmark)
                    .ok_or("visible add button")?;
                assert_eq!(add.enabled, !shared);
                assert!(!ui.daily_rows().is_empty());
                assert_eq!(
                    matches!(
                        ui.command(Command::AddBookmark, &mut keymap, &mut profile),
                        Some(UiResult::AddBookmark)
                    ),
                    !shared
                );
                assert!(ui.daily_command(true).is_none());
                assert!(ui.daily_command(false).is_none());
                assert!(ui.daily_activate().is_none());
            }
        }
    }
    Ok(())
}
