# Changelog

Versions follow MAJOR.MINOR.PATCH. Before 1.0: MINOR goes up when something new ships, PATCH when
there are only fixes.

## [0.4.1] - 2026-11-20

### Changed
- No visible changes. This release checks the in-app updater from start to finish: if you have
  0.4.0, Chronn should offer this version, install it and reopen on its own.

## [0.4.0] - 2026-11-20

First alpha release.

### Added
- English and Portuguese (Brazil). Settings → Language: System (default), Português (Brasil) or
  English. Changes apply right away in every window, the dock and the tray. The installer
  follows the Windows language.
- Updates from inside the app. Once a day, when it opens, Chronn asks GitHub for a new version
  (the only time it uses the internet; turn it off in Settings → About). When one is out, an
  icon shows in the header: one click downloads it, checks its signature, installs and reopens.
  Settings → About also has "Check for updates".
- Rename a group straight from its ⋯ menu.
- Settings → Dock: how the collapsed dock shows a timer's space (only the timer's color, the
  space name as text, or the space color).
- "alpha" next to the version in Settings → About and in the tray.

### Changed
- The collapsed dock shows a single dot per timer; the space is in the dot's tooltip.
- Dates and durations follow the language.

## [0.3.1] - 2026-11-20

### Changed
- Header, first row: "+ New" is now the main button, next to "Log the day" (coming soon), Help and
  Settings, all on the left. The day total stands alone on the right.
- Header, second row: a screens menu (Panel, Focus, Cards, Timeline) with the current screen
  checked, then the space tabs. "N running", "Pause all" and "Reset all" become quiet text buttons
  on the right.
- On narrow windows the header compacts step by step instead of cutting or overlapping.
- Small menus work with the arrow keys, Home and End, and show a visible focus outline.

### Added
- A tour step that shows the screens menu.

## [0.3.0] - 2026-11-20

### Added
- Move a timer or a group to another space (from the ⋯ menu or the timer's Group field, now
  grouped by space). Totals follow; Undo for 10 s.
- Settings → Appearance: Light, Dark or System (default).
- Settings → Panel: show or hide the timeline on the Panel; it also collapses like a group.

### Changed
- New two-row header: big day total and date on the right, navigation, space tabs and actions on
  the second row, and a prominent "+ New" button.
- Focus view lists every activity of the timer.
- Selectors always keep the current value, even when it is archived.

## [0.2.0] - 2026-11-20

### Added
- Spaces above groups (for example Work and Personal), shown as tabs once there are two or more,
  with an "All" tab. Day total, "Pause all" and "Reset all" follow the open tab.
- A single "+ New" button (or the N key) to create a timer, group or space; a timer created without
  a group goes to "No group".
- Choose which activities show in each timer's panel. A hidden activity with time, or running,
  still shows.
- Reset a single activity (for example only Meeting) from the timer's reset menu.

### Changed
- The timer panel lists every non-archived activity, including Study.
- Long tooltips wrap into lines instead of being cut at the screen edge.
- A running timer is never hidden, even inside an archived group or space.

### Removed
- Import from the previous internal version.

## [0.1.0] - 2026-11-20

First public version.

### Added
- Groups and timers with play, pause, stop and activities (Work, Meeting, Study).
- Dock always in view (or only when there is a question), with notices in their own window.
- Claude Code connection through hooks: a conversation counts in its folder's timer; a new one asks.
- Recent projects: first-run setup finds the folders where Claude Code worked.
- Pause on inactivity, "waiting for you" and "background" reminders.
- Reset a timer, a single activity or all of them, without deleting history.
- First-run setup, essential and full tours, per-screen help and tips.
- Automatic backups and restore.
