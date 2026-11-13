# Window

What a cattery window is and what happens to the session it shows.

## Opening

Starting cattery opens one window showing a new session, which runs the
user's login shell in the directory cattery was started from, with
cattery's environment.

The session lives in cattery's own felis daemon. cattery starts that daemon
when none is running, and it keeps running after the window closes. It is
apart from any felis daemon the user runs: a felis client does not see
cattery's sessions, and cattery does not see theirs.

## While it is open

The window shows the session's screen as felis draws it, and every key
typed in the window reaches the session. Resizing the window resizes the
session to the cells that fit.

## Closing

A window closes when the program in its session ends, and closing the
window ends the session.
