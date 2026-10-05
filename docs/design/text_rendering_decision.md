# Current text rendering decision

Phase 1F replaces the Phase 1E smooth note subset with existing Spleen bitmap
cells and the existing lazy Unifont fallback. UI and notes share glyph lookup,
integer logical pixel presentation and hard binary edges. The old SDF atlas,
font texture, Liberation-derived assets and generator are removed.

See [the current font decision](ui_font_decision.md) for exact sources, rights,
coverage, sizes, bounded costs and the shaping boundary. Plain UTF-8, scalar
wrapping, tabs, alignment and box clipping remain. Complex shaping/bidi and rich
text are deferred. The [Phase 1E report](../MISSION_1E_REPORT.md) records the
historical measurements of the replaced rendering path; they do not describe
the current renderer.
