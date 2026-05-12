# installer/

Branded assets for the Magpie macOS DMG installer.

## Files

| file                    | role                                                                                                            |
| ----------------------- | --------------------------------------------------------------------------------------------------------------- |
| `dmg-background.svg`    | single source of truth for the installer background design                                                      |
| `dmg-background.png`    | rendered @1x (660×400) — used by Finder on non-retina displays                                                  |
| `dmg-background@2x.png` | rendered @2x (1320×800) — used by Finder on retina displays                                                     |
| `dmg-background.tiff`   | multi-rep TIFF combining both PNGs — what `create-dmg` actually consumes                                        |
| `volume-icon.icns`      | optional custom volume icon shown when the DMG is mounted (falls back to `src-tauri/icons/icon.icns` if absent) |
| `build-assets.sh`       | regenerates the PNGs + TIFF from the SVG                                                                        |

## Editing the design

Edit `dmg-background.svg`, then run:

```bash
bash installer/build-assets.sh
```

You will need `rsvg-convert` (`brew install librsvg`). `tiffutil` ships with macOS.

The result is committed to the repo, so the build script (`scripts/build-macos.sh`) does **not** invoke this script — it just consumes the pre-rendered `.tiff`.

## Layout contract

Window dimensions and icon positions are duplicated in three places and must stay in sync:

1. `installer/dmg-background.svg` — visual layout assumes window size 660×400 and icon centers at (180, 170) and (480, 170)
2. `scripts/build-macos.sh` — passes `--window-size 660 400`, `--icon "Magpie.app" 180 170`, `--app-drop-link 480 170` to `create-dmg`
3. `src-tauri/tauri.conf.json` (`bundle.macOS.dmg` block) — kept as documentation; not actually consumed because the build script bypasses Tauri's DMG bundler

If you change one, change all three.

## Palette

Colors come from `src/styles/global.css` ("Warm Studio" theme tokens). Don't introduce new colors here — pull from the existing tokens so the DMG never drifts from the app:

- Canvas: `#131211` → `#1b1917` (vertical gradient)
- Gold accent: `#e8af47` → `#b8862e`
- Off-white ink: `#ede8e2`
- Muted ink: `#9e978f`
- Edge: `#332f2b`
