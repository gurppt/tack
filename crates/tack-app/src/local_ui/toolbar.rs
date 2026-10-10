//! Two bounded lists and ordinary buttons; the same icon atlas as the bar.
use super::*;
use crate::toolbar::Item;
impl LocalUi {
    fn toolbar_actions() -> Vec<Item> {
        std::iter::once(Item::Separator)
            .chain(
                Action::ALL
                    .into_iter()
                    .filter(|a| crate::toolbar::eligible(*a))
                    .map(Item::Action),
            )
            .collect()
    }
    pub(super) fn toolbar_column(&mut self, order: bool) {
        self.toolbar_selected[usize::from(self.toolbar_order)] = self.selected;
        self.toolbar_first[usize::from(self.toolbar_order)] = self.first;
        self.toolbar_order = order;
        self.selected = self.toolbar_selected[usize::from(order)];
        self.first = self.toolbar_first[usize::from(order)];
        self.focus = None;
    }
    pub(super) fn draw_toolbar(&mut self, g: &mut ImageGizmo, c: &Camera, p: &Preferences) {
        let s = c.ui_scale().round().clamp(1., 4.);
        let shell = crate::modal_shell::ModalShell::work(c, [600., 440.]);
        let [_, _, width, height] = shell.rect;
        let palette = p.theme.palette();
        let mut budget = 900;
        self.layout = [s, width, height, 76.];
        self.visible = ((height - 146.) / 22.).floor().clamp(1., 12.) as usize;
        self.icon_count = 0;
        shell.paint(g, c, palette);
        g.ui_text(
            c,
            [24. * s, 20. * s],
            width - 24.,
            "Edit Toolbar",
            palette.accent_primary,
            &mut budget,
        );
        let title = format!(
            "{} / {} of 32",
            p.toolbar.placement.label(),
            p.toolbar.actions.len()
        );
        g.ui_text(
            c,
            [24. * s, 40. * s],
            width - 24.,
            &title,
            palette.text_secondary,
            &mut budget,
        );
        let mid = (width + 24.) / 2.;
        g.ui_text(
            c,
            [24. * s, 58. * s],
            mid - 24.,
            "Available",
            palette.accent_secondary,
            &mut budget,
        );
        g.ui_text(
            c,
            [(mid + 8.) * s, 58. * s],
            width - mid - 8.,
            "Order",
            palette.accent_secondary,
            &mut budget,
        );
        let actions = Self::toolbar_actions();
        let length = if self.toolbar_order {
            p.toolbar.actions.len()
        } else {
            actions.len()
        };
        self.selected = self.selected.min(length.saturating_sub(1));
        // Scroll the smallest amount needed, retaining each column's position.
        self.first = self.first.min(length.saturating_sub(self.visible));
        if self.reveal_row {
            self.first = crate::ui_scroll::reveal(self.first, self.selected, self.visible, length);
        }
        self.reveal_row = false;
        self.toolbar_first[usize::from(self.toolbar_order)] = self.first;
        for right in [false, true] {
            let list: Vec<_> = if right {
                p.toolbar
                    .actions
                    .iter()
                    .filter_map(|id| Item::from_id(id))
                    .collect()
            } else {
                actions.clone()
            };
            let first = self.toolbar_first[usize::from(right)];
            let x = if right { mid + 8. } else { 24. };
            let available = if right { width - x } else { mid - x - 8. };
            for (index, item) in list.iter().enumerate().skip(first).take(self.visible) {
                let y = 76. + (index - first) as f64 * 22.;
                let selected = self.toolbar_order == right && index == self.selected;
                if selected {
                    g.pixel_rect(
                        c,
                        [(x - 4.) * s, y * s],
                        [(x + available - 8.) * s, (y + 20.) * s],
                        if self.feedback.active() && right {
                            [0., 0.5, 0.08, 1.]
                        } else {
                            palette.selection
                        },
                        None,
                    );
                    // Pixel focus mark remains legible on every palette.
                    g.pixel_rect(
                        c,
                        [(x - 4.) * s, y * s],
                        [(x - 3.) * s, (y + 20.) * s],
                        palette.accent_secondary,
                        None,
                    );
                }
                let icon = item.icon();
                if let Some(index) = icon
                    && self.icon_count < self.icons.len()
                {
                    self.icons[self.icon_count] = tack_render::UiIcon {
                        rect: [x * s, (y + 2.) * s, 16. * s, 16. * s],
                        index,
                        disabled: false,
                    };
                    self.icon_count += 1;
                }
                let inset = if icon.is_some() { 20. } else { 0. };
                g.ui_text(
                    c,
                    [(x + inset) * s, (y + 2.) * s],
                    available - inset - 8.,
                    item.label(),
                    palette.text_primary,
                    &mut budget,
                );
            }
            self.scroll[usize::from(right)].draw(
                g,
                c,
                palette,
                [x - 4., 76., x + available, 76. + self.visible as f64 * 22.],
                (first, self.visible, list.len()),
            );
        }
        let hint = if width < 420. {
            "Ctrl+Up/Down: reorder"
        } else {
            "Tab: focus  Ctrl+Up/Down: reorder"
        };
        g.ui_text(
            c,
            [24. * s, (height - 70.) * s],
            width - 24.,
            hint,
            palette.accent_attention,
            &mut budget,
        );
        self.hits.clear();
        let labels = [
            ("Add", Command::ToolbarToggle),
            ("Remove", Command::ToolbarRemove),
            ("Up", Command::ToolbarUp),
            ("Down", Command::ToolbarDown),
            ("Position", Command::ToolbarPlacement),
            ("Scale 1/2/3", Command::ToolbarScale),
            ("Reset", Command::ToolbarReset),
            ("Done", Command::Close),
        ];
        for (i, (label, command)) in labels.into_iter().enumerate() {
            let x = 24. + (i % 4) as f64 * (width - 32.) / 4.;
            let y = height - 48. + (i / 4) as f64 * 24.;
            let x2 = x + (width - 32.) / 4. - 4.;
            let enabled = match command {
                Command::ToolbarToggle => p.toolbar.actions.len() < 32,
                Command::ToolbarRemove | Command::ToolbarUp | Command::ToolbarDown => {
                    !p.toolbar.actions.is_empty()
                }
                _ => true,
            };
            self.hits.push(Hit {
                rect: [x, y, x2, y + 20.],
                command,
                enabled,
            });
            let focused = self.focus == Some(command);
            g.pixel_rect(
                c,
                [x * s, y * s],
                [x2 * s, (y + 20.) * s],
                if focused {
                    palette.accent_primary
                } else {
                    palette.selection
                },
                None,
            );
            g.ui_text(
                c,
                [(x + 2.) * s, (y + 2.) * s],
                x2 - x - 4.,
                label,
                if !enabled {
                    palette.text_disabled
                } else if focused {
                    palette.menu_bg
                } else {
                    palette.text_primary
                },
                &mut budget,
            );
        }
    }
    pub(super) fn toolbar_command(
        &mut self,
        cmd: Command,
        p: &mut Preferences,
    ) -> Option<UiResult> {
        let item = if self.toolbar_order {
            p.toolbar
                .actions
                .get(self.selected)
                .and_then(|id| Item::from_id(id))
        } else {
            Self::toolbar_actions().get(self.selected).copied()
        };
        let position = if self.toolbar_order {
            (self.selected < p.toolbar.actions.len()).then_some(self.selected)
        } else {
            item.and_then(|item| p.toolbar.actions.iter().position(|id| *id == item.id()))
        };
        self.focus = None;
        self.reveal_row = true;
        match cmd {
            Command::ToolbarRemove => {
                if let Some(i) = position {
                    p.toolbar.actions.remove(i);
                    self.toolbar_column(true);
                    self.selected = i.min(p.toolbar.actions.len().saturating_sub(1));
                }
            }
            Command::ToolbarToggle => {
                if p.toolbar.actions.len() < 32
                    && let Some(item) = item
                    && (item == Item::Separator || position.is_none())
                {
                    if item == Item::Separator
                        && p.toolbar
                            .actions
                            .last()
                            .is_some_and(|id| id == crate::toolbar::SEPARATOR)
                    {
                        return None;
                    }
                    p.toolbar.actions.push(item.id());
                    self.toolbar_column(true);
                    self.selected = p.toolbar.actions.len() - 1;
                    self.feedback.acknowledge(std::time::Instant::now());
                }
            }
            Command::ToolbarUp | Command::ToolbarDown => {
                if let Some(i) = position {
                    let next = if cmd == Command::ToolbarUp {
                        i.checked_sub(1)
                    } else {
                        (i + 1 < p.toolbar.actions.len()).then_some(i + 1)
                    };
                    if let Some(next) = next {
                        p.toolbar.actions.swap(i, next);
                        self.toolbar_column(true);
                        self.selected = next;
                    }
                }
            }
            Command::ToolbarScale => {
                p.toolbar.scale = p.toolbar.scale % 3 + 1;
            }
            Command::ToolbarPlacement => {
                let all = crate::toolbar::Placement::ALL;
                let i = all
                    .iter()
                    .position(|v| *v == p.toolbar.placement)
                    .unwrap_or(0);
                p.toolbar.placement = all[(i + 1) % all.len()];
            }
            Command::ToolbarReset => {
                p.toolbar = crate::toolbar::Config::default();
                self.toolbar_column(true);
                self.selected = 0;
                self.first = 0;
            }
            _ => return None,
        }
        Some(UiResult::PreferencesChanged)
    }
    pub(super) fn toolbar_count() -> usize {
        Self::toolbar_actions().len()
    }
}
