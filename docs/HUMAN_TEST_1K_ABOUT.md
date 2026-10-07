# Human check — About addendum

Run `./bin/tack` after `bash tools/build-test-bin.sh`. Keep `bin/tack-about.png`
beside it. The automated checks use a separate display and do not replace your
visual/comfort review.

- Open About Tack through F10, then through right-click → Tack. Both should show
  the same compact flat panel and cowboy artwork on the right.
- At 800×600, test Preferences UI scale 1× and 2× with each background theme.
  Read the whole author and URL; inspect image proportions and bitmap borders.
  Confirm Contact is “Not set” and License “Undecided”.
- Dismiss using Escape, Enter and Close; open/close several times and resume
  normal board editing. There should be no animation or continuous redraw.
- If needed, edit `gfx/about.toml` and rebuild. Changing `version` to a value
  different from Cargo must fail explicitly; omit it to use Cargo's version.
- To test missing artwork, use a disposable copy of the package, removing only
  that copy's `tack-about.png`. About should retain readable text and Close.
  Preserve your ordinary package and source artwork.

Owner visual acceptance and independent review remain pending. Linux/X11 has
automated native coverage; Windows builds are checked in CI, with no claim of a
real Windows desktop About session.
