# Changelog

Versions follow MAJOR.MINOR.PATCH. Before 1.0: MINOR goes up when something new ships, PATCH when
there are only fixes.

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
