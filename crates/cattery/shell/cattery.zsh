# Marks prompts and commands (OSC 133) and reports the working directory
# (OSC 7), so Cattery can jump between prompts and open a window where the
# shell is.

[[ -o interactive ]] || return 0
(( ${+_cattery_running} )) && return 0
typeset -gi _cattery_running=0
unset CATTERY_SHELL_INTEGRATION

autoload -Uz add-zsh-hook

_cattery_preexec() {
  _cattery_running=1
  print -n '\e]133;C\e\\'
}

_cattery_mark_end() {
  local code=$?
  # An empty command line ran nothing, so there is nothing to end.
  (( _cattery_running )) || return 0
  _cattery_running=0
  print -n "\e]133;D;$code\e\\"
}

_cattery_mark_prompt() {
  # Written into PS1 rather than printed: a repaint after a resize prints
  # PS1 alone.
  [[ $PS1 == *$'\e]133;A'* ]] || PS1=$'%{\e]133;A\e\\%}'$PS1
  [[ $PS1 == *$'\e]133;B'* ]] || PS1+=$'%{\e]133;B\e\\%}'
}

_cattery_report_cwd() {
  local LC_ALL=C letter encoded=
  for letter in ${(s::)PWD}; do
    if [[ $letter == [[:alnum:]/._~-] ]]; then
      encoded+=$letter
    else
      encoded+=%${(l:2::0:)$(( [##16] #letter ))}
    fi
  done
  print -n "\e]7;file://${HOST}${encoded}\e\\"
}

# zsh 5.10 marks prompts itself, without the exit status.
eval 'typeset -ga .term.extensions' 2>/dev/null && .term.extensions+=(-integration)

add-zsh-hook preexec _cattery_preexec
add-zsh-hook precmd _cattery_mark_end
add-zsh-hook precmd _cattery_mark_prompt
add-zsh-hook chpwd _cattery_report_cwd
_cattery_report_cwd
