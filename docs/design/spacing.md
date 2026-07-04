# Spacing

## The scale

Extended from the original 4-rung system (`4·8·12·16`) to a full 7-rung scale:

| Token | Value | Use |
|---|---|---|
| `SPACE_XS` | 4px | Between a label and an inline icon/badge |
| `SPACE_SM` | 8px | Between adjacent controls in a toolbar/row; default panel content padding (`PANEL_PADDING`) |
| `SPACE_MD` | 12px | Between sections within a panel |
| `SPACE_LG` | 16px | Between major panel regions |
| `SPACE_XL` | 24px *(new)* | Gutter between docked panels (Sidebar↔Viewport, Viewport↔Neural Viewer) |
| `SPACE_XXL` | 32px *(new)* | Dialog/modal outer padding |
| `SPACE_XXXL` | 48px *(new)* | Empty-state vertical centering offset |

The original scale stopped at 16, so every larger gap in the app (dialog padding, empty-state centering, panel gutters) either reused 16 or fell back to an unstyled default. The three new rungs each have one specific, named job — they aren't generic "bigger" options.

## Panel padding is not one-size-fits-all

`PANEL_PADDING` (= `SPACE_SM`, 8px) is currently applied uniformly to every docked panel's content (`layout.rs::pane_ui`), regardless of what that panel holds — a dense Inspector data grid, a Neural Viewer graph canvas, and a four-plot Metrics dashboard all get the identical 8px inset today. Going forward:

- **Dense/reading panels** (Inspector, Sidebar tabs): `SPACE_MD` (12px) — more breathing room for text-heavy content.
- **Canvas panels** (Neural Viewer, Metrics plot area): `SPACE_XS` or `SPACE_SM` (4-8px) — reclaim space for the graph itself.
- **Dialogs**: `SPACE_XXL` (32px) outer padding.

## Chrome height

`CHROME_HEIGHT` (22px) is unchanged — it's already applied consistently across all three chrome-bar implementations and is being consolidated into one `chrome_bar` component (see [`components.md`](components.md)), not re-tuned.

## Radius and elevation

Not strictly spacing, but governed by the same "one named tier, not an arbitrary literal" rule:

| Token | Radius | Use |
|---|---|---|
| `RADIUS_TIGHT` | 4px | Tooltips, graph canvases (Neural Viewer, Metrics plot backgrounds) |
| `RADIUS_STD` | 8px | Floating windows, toasts, context menus |
| `RADIUS_LOOSE` | 12px | Dialogs/modals |

Each radius tier has a paired shadow/elevation constant, so "how much does this float above the surface" is as tokenized as "how rounded are its corners."
