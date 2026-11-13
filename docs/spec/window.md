# Window

What a cattery window is and what happens to the session it shows.

## Opening

Starting Cattery opens one window showing a new session, which runs the
user's login shell in the directory Cattery was started from, with
Cattery's environment.

The session lives in Cattery's own felis daemon. Cattery starts that daemon
when none is running, and it keeps running after the window closes. It is
apart from any felis daemon the user runs: a felis client does not see
Cattery's sessions, and Cattery does not see theirs.

## While it is open

The window shows the session's screen as felis draws it, and every key
typed in the window reaches the session. Text typed through the platform's
input method is shown at the cursor while it is composed, with the input
method's candidates beside it, and reaches the session once committed.
Resizing the window resizes the session to the cells that fit.

## Closing

A window closes when the program in its session ends, and closing the
window ends the session.
