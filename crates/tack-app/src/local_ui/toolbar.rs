//! Simple catalog checkbox/reorder editor, reusing temporary panel primitives.
use super::*;
impl LocalUi {
    fn toolbar_actions() -> Vec<Action> {
        Action::ALL
            .into_iter()
            .filter(|a| crate::toolbar::eligible(*a))
            .collect()
    }
    pub(super) fn draw_toolbar(&mut self, g: &mut ImageGizmo, c: &Camera, p: &Preferences) {
        let s = c.ui_scale();
        let width = (f64::from(c.screen_size()[0]) / s - 24.).clamp(1., 600.);
        let height = (f64::from(c.screen_size()[1]) / s - 24.).clamp(1., 440.);
        let mut budget = 900;
        self.layout = [s, width, height, 70.];
        self.visible = ((height - 160.) / 22.).floor().clamp(1., 12.) as usize;
        g.pixel_rect(
            c,
            [12. * s, 12. * s],
            [(width + 12.) * s, (height + 12.) * s],
            p.theme.palette().menu_bg,
            None,
        );
        g.ui_text(
            c,
            [24. * s, 22. * s],
            width - 24.,
            "Edit Toolbar - select action, Add/Remove",
            p.theme.palette().accent_primary,
            &mut budget,
        );
        let title = format!(
            "Position: {}  /  {} of 32",
            p.toolbar.placement.label(),
            p.toolbar.actions.len()
        );
        g.ui_text(
            c,
            [24. * s, 44. * s],
            width - 24.,
            &title,
            p.theme.palette().text_primary,
            &mut budget,
        );
        let actions = Self::toolbar_actions();
        self.first = self
            .selected
            .saturating_sub(self.visible / 2)
            .min(actions.len().saturating_sub(self.visible));
        for (i, a) in actions
            .iter()
            .enumerate()
            .skip(self.first)
            .take(self.visible)
        {
            let y = 70. + (i - self.first) as f64 * 22.;
            if i == self.selected {
                g.pixel_rect(
                    c,
                    [20. * s, y * s],
                    [(width + 4.) * s, (y + 20.) * s],
                    p.theme.palette().selection,
                    None,
                );
            }
            let label = if let Some(pos) = p.toolbar.actions.iter().position(|id| *id == a.id()) {
                format!("{:02}  {}", pos + 1, a.label())
            } else {
                format!("--  {}", a.label())
            };
            g.ui_text(
                c,
                [24. * s, (y + 2.) * s],
                width - 24.,
                &label,
                p.theme.palette().text_primary,
                &mut budget,
            );
        }
        self.hits.clear();
        let labels = [
            ("Add/Remove", Command::ToolbarToggle),
            ("Up", Command::ToolbarUp),
            ("Down", Command::ToolbarDown),
            ("Position", Command::ToolbarPlacement),
            ("Reset", Command::ToolbarReset),
            ("Done", Command::Close),
        ];
        for (i, (label, command)) in labels.into_iter().enumerate() {
            let row = i / 3;
            let x = 24. + (i % 3) as f64 * (width - 32.) / 3.;
            let y = height - 54. + row as f64 * 26.;
            let x2 = x + (width - 32.) / 3. - 4.;
            self.hits.push(Hit {
                rect: [x, y, x2, y + 22.],
                command,
                enabled: true,
            });
            g.pixel_rect(
                c,
                [x * s, y * s],
                [x2 * s, (y + 22.) * s],
                p.theme.palette().selection,
                None,
            );
            g.ui_text(
                c,
                [x * s, (y + 3.) * s],
                x2 - x,
                label,
                p.theme.palette().text_primary,
                &mut budget,
            );
        }
    }
    pub(super) fn toolbar_command(
        &mut self,
        cmd: Command,
        p: &mut Preferences,
    ) -> Option<UiResult> {
        let a = Self::toolbar_actions().get(self.selected).copied();
        let id = a.map(Action::id);
        let position = id
            .as_ref()
            .and_then(|id| p.toolbar.actions.iter().position(|v| v == id));
        match cmd {
            Command::ToolbarToggle => {
                if let Some(i) = position {
                    p.toolbar.actions.remove(i);
                } else if p.toolbar.actions.len() < 32
                    && let Some(id) = id
                {
                    p.toolbar.actions.push(id);
                }
            }
            Command::ToolbarUp => {
                if let Some(i) = position
                    && i > 0
                {
                    p.toolbar.actions.swap(i, i - 1);
                }
            }
            Command::ToolbarDown => {
                if let Some(i) = position
                    && i + 1 < p.toolbar.actions.len()
                {
                    p.toolbar.actions.swap(i, i + 1);
                }
            }
            Command::ToolbarPlacement => {
                let all = crate::toolbar::Placement::ALL;
                let i = all
                    .iter()
                    .position(|v| *v == p.toolbar.placement)
                    .unwrap_or(0);
                p.toolbar.placement = all[(i + 1) % all.len()];
            }
            Command::ToolbarReset => p.toolbar = crate::toolbar::Config::default(),
            _ => return None,
        }
        Some(UiResult::PreferencesChanged)
    }
    pub(super) fn toolbar_count() -> usize {
        Self::toolbar_actions().len()
    }
}
