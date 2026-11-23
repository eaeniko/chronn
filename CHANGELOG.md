# Changelog

Versions follow MAJOR.MINOR.PATCH. Before 1.0: MINOR goes up when something new ships, PATCH when
there are only fixes.

## [0.6.1] - 2026-11-23

This release also brings 0.6.0, which wasn't released on its own: Claude Code in WSL, reading in
the terminal by window, and Claude accounts linked to a space.

### Fixed
- Terminal sessions (Windows and WSL) now show their name: without a name you gave, Chronn uses
  the title Claude Code generates for the session.
- Terminal sessions now know their Claude account, read from the session itself. Chronn still
  keeps only a short digest of the account id, never the id.
- Accounts now say which one they are: the name, a masked e-mail and the plan
  ("Ana · an•••@example.com · Pro"), in "Which space is this account in?" and under Settings.
- The dock no longer swaps one timer for another: a session you were just reading stays in the
  dock as "waiting for you" while you read another one.
- Events from WSL arrive in about 2 seconds during a session (they could take over 30 seconds).
- "Connect" also works on a stopped distro: Chronn starts it only when you click. The WSL section
  under Settings now updates on its own while Settings is open.
- "Always for this folder" warns when the folder is your user folder or holds several projects,
  and tells you to open `claude` inside the project's folder instead.
- If another program rewrites a distro's Claude Code settings without Chronn's hooks, Settings
  and the main window now say so, with "Connect again". It shows the exact change first and
  keeps everything else in the file.

### Known limits
- With the distro stopped, its last events wait in its queue until it runs again.

## [0.6.0] - 2026-11-22

### Added
- Claude Code in WSL now counts. Under Settings, "Claude Code in WSL" lists your distros: link
  one and Chronn shows the exact change to that distro's Claude Code settings first. A small
  Linux hook writes the events inside the distro, and Chronn reads them from Windows, so nothing
  in Linux needs to run Windows programs (it works with interop turned off). Chronn never wakes
  a distro that isn't running.
- Reading in the terminal without the session name in the title: when you send a message from
  Windows Terminal or VS Code, Chronn remembers that window (in memory only) and counts the time
  you spend in it as reading that session.
- Claude accounts: with two or more spaces, Chronn asks once which space a new Claude account
  belongs to. "Link a session" then lists the sessions of the timer's space first and folds the
  others under "Other accounts". Chronn keeps only a short digest of the account id, never the
  id. Manage them under Settings, "Claude accounts".

## [0.5.0] - 2026-11-21

### Added
- Chat conversations in the Claude app now count, like Claude Code sessions: Chronn sees which
  conversation is open, shows "reading" and "writing", and notices when Claude is answering (by
  the "Stop response" button and the "Running" mark in the sidebar). Link a conversation to a
  timer like any session. Chronn keeps only the conversation's code, never its title: after
  Chronn restarts, a linked conversation shows as "Chat conversation" until you open it again.
- The "Which timer should count it?" question is bigger and clearer: the session name, where it
  comes from (Claude app, terminal, VS Code or Chat) and its folder, each with a label.

### Changed
- The main button links only this session. To make every new session in a folder count on its
  own, tick "Always for this folder" and pick the folder, from where the session started to
  where it is now.
- "writing" shows only after you type something (the send button is active), not as soon as a
  conversation opens.
- The list of sessions to link shows the free ones first and the linked ones last, with where
  each one comes from.

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
