# Cattery starts fish with its own folder first on XDG_DATA_DIRS, so fish
# loads this file. The user's XDG_DATA_DIRS goes back first.
if set -q CATTERY_XDG_DATA_DIRS
    set -gx XDG_DATA_DIRS $CATTERY_XDG_DATA_DIRS
    set -e CATTERY_XDG_DATA_DIRS
else
    set -e XDG_DATA_DIRS
end

status is-interactive; or exit

# fish marks prompts (OSC 133) and reports its directory (OSC 7) itself.
# What it does not report is the command line, which goes out as felis's
# program status (OSC 7501), so Cattery can say what last ran in a session.
function __cattery_report_command --argument-names state command
    set -l line (string replace -ra '[[:cntrl:][:space:]]+' ' ' -- $command | string trim)
    # felis keeps 192 bytes of a status title; 47 letters of up to four
    # bytes each, and the ellipsis, fit.
    if test (string length -- $line) -gt 47
        set line (string sub -l 47 -- $line)…
    end
    set -l title (printf '%s' $line | base64 | string join '')
    printf '\e]7501;state=%s:id=shell:app=cattery:title=%s\e\\' $state $title
end

function __cattery_preexec --on-event fish_preexec
    __cattery_report_command working $argv[1]
end

function __cattery_postexec --on-event fish_postexec
    set -l code $status
    if test $code -eq 0
        __cattery_report_command done $argv[1]
    else
        __cattery_report_command error $argv[1]
    end
end
