use tack_app::{
    clipboard,
    preferences::Preferences,
    ui_theme::{Palette, Theme},
};
use tack_core::*;
type R<T = ()> = Result<T, tack_assets::AssetError>;
fn luminance(c: [f32; 4]) -> f32 {
    c[0] * 0.2126 + c[1] * 0.7152 + c[2] * 0.0722
}
fn contrast(a: [f32; 4], b: [f32; 4]) -> f32 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}
#[test]
fn mime_priority_and_editor_text_override_are_deterministic() {
    let types = "text/plain\nimage/png\nUTF8_STRING\ntext/uri-list\nimage/jpeg\n";
    assert_eq!(clipboard::choose_mime(types, false), Some("text/uri-list"));
    assert_eq!(clipboard::choose_mime(types, true), Some("UTF8_STRING"));
    assert_eq!(
        clipboard::choose_mime("image/png\nimage/jpeg", false),
        Some("image/png")
    );
    assert_eq!(
        clipboard::choose_mime("application/octet-stream", false),
        None
    );
    assert_eq!(clipboard::choose_mime("image/png", true), None);
}
#[test]
fn plain_text_leading_slash_is_not_mistaken_for_files() -> R {
    for text in [
        "/usr/bin/example --flag",
        "/review notes",
        "/nonexistent/review.png",
    ] {
        let clipboard::Clipboard::Text(result) = clipboard::normalize_text(text.into(), false)?
        else {
            return Err("ordinary text was mistaken for a file list".into());
        };
        assert_eq!(result, text);
    }
    let path = std::env::temp_dir().join(format!("tack-clipboard-text-{}.PNG", std::process::id()));
    let file = std::fs::File::options()
        .write(true)
        .create_new(true)
        .open(&path)?;
    drop(file);
    let result = clipboard::normalize_text(path.to_string_lossy().into_owned(), false);
    let editor = clipboard::normalize_text(path.to_string_lossy().into_owned(), true);
    std::fs::remove_file(&path)?;
    let clipboard::Clipboard::Files { paths, rejected } = result? else {
        return Err("legacy copied absolute image path should remain supported".into());
    };
    assert_eq!(paths, [path]);
    assert_eq!(rejected, 0);
    assert!(matches!(editor?, clipboard::Clipboard::Text(_)));
    Ok(())
}
#[test]
fn mixed_local_list_keeps_valid_entries_order_and_rejects_remote() -> R {
    #[cfg(unix)]
    let (first, last) = ("file:///tmp/a%20b.png", "file:///tmp/c.jpg");
    #[cfg(windows)]
    let (first, last) = ("file:///C:/temp/a%20b.png", "file:///C:/temp/c.jpg");
    let text = format!(
        "copy\n{first}\nhttps://example.test/no.png\nfile://remote/no.png\n{last}\nfile:///tmp/%00.png\n"
    );
    let (paths, rejected) = clipboard::references(&text)?;
    assert_eq!(rejected, 3);
    assert_eq!(paths.len(), 2);
    assert_eq!(
        paths[0].file_name().and_then(|s| s.to_str()),
        Some("a b.png")
    );
    assert_eq!(paths[1].file_name().and_then(|s| s.to_str()), Some("c.jpg"));
    assert!(clipboard::references("https://example.test/no.png").is_err());
    assert!(clipboard::references(&"x".repeat(65537)).is_err());
    assert!(clipboard::references(&format!("{first}\n").repeat(4097)).is_err());
    Ok(())
}
#[test]
fn three_flat_palettes_have_legible_ui_states() {
    for theme in Theme::ALL {
        let p = theme.palette();
        for channel in 0..3 {
            assert!(p.background[channel] > 0. && p.background[channel] < 1.);
        }
        for background in [p.menu_bg, p.selection] {
            for text in [
                p.text_primary,
                p.text_secondary,
                p.accent_primary,
                p.accent_secondary,
                p.accent_attention,
            ] {
                assert!(
                    contrast(text, background) >= 4.5,
                    "{theme:?}: {}",
                    contrast(text, background)
                );
            }
            assert!(
                contrast(p.text_disabled, background) >= 3.,
                "disabled {theme:?}"
            );
        }
        let _: Palette = p;
    }
}
#[test]
fn old_profiles_default_neutral_unknown_future_theme_is_rejected() -> R {
    let mut profile = Preferences::defaults()?;
    for theme in Theme::ALL {
        profile.theme = theme;
        profile.ui_scale = 2;
        let parsed: Preferences = serde_json::from_slice(&serde_json::to_vec(&profile)?)?;
        assert_eq!(parsed.theme, theme);
        assert_eq!(parsed.ui_scale, 2);
        parsed.keymap()?;
    }
    let mut value = serde_json::to_value(&profile)?;
    value.as_object_mut().ok_or("profile")?.remove("theme");
    assert_eq!(
        serde_json::from_value::<Preferences>(value.clone())?.theme,
        Theme::NeutralGray
    );
    value["theme"] = serde_json::json!("FutureTheme");
    assert!(serde_json::from_value::<Preferences>(value).is_err());
    profile.ui_scale = 255;
    assert!(profile.keymap().is_err());
    Ok(())
}
#[test]
fn canvas_text_paste_uses_note_history_and_theme_changes_leave_document_intact() -> R {
    let mut editor = DocumentEditor::new(
        Document::new(DocumentId::new(1)?, DocumentLimits::default()),
        200,
    );
    let mut input = tack_app::image_input::ImageInput::new()?;
    let camera = Camera::new([800, 600]);
    let empty = editor.document().clone();
    input.paste_text_note("Hello\nclipboard".into(), &mut editor, &camera)?;
    assert_eq!(editor.document().annotation_count(), 1);
    assert_eq!(editor.undo_len(), 1);
    let before = editor.document().clone();
    for theme in Theme::ALL {
        input.gizmo.palette = theme.palette();
        input.build_overlay(&editor, &camera);
        assert_eq!(editor.document(), &before);
    }
    assert!(
        input
            .paste_text_note("bad\0text".into(), &mut editor, &camera)
            .is_err()
    );
    assert_eq!(editor.document(), &before);
    editor.undo()?;
    assert_eq!(editor.document(), &empty);
    Ok(())
}
