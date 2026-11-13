# Marks prompts and commands (OSC 133), reports the working directory
# (OSC 7), and reports each command line as felis's program status
# (OSC 7501), so Cattery can jump between prompts, open a window where the
# shell is, and say what last ran in a session.

[[ -o interactive ]] || return 0
(( ${+_cattery_running} )) && return 0
typeset -gi _cattery_running=0
typeset -g _cattery_command=
unset CATTERY_SHELL_INTEGRATION

autoload -Uz add-zsh-hook

_cattery_preexec() {
  _cattery_running=1
  print -n '\e]133;C\e\\'
  _cattery_command=$1
  _cattery_report_command working
}

_cattery_mark_end() {
  local code=$?
  # An empty command line ran nothing, so there is nothing to end.
  (( _cattery_running )) || return 0
  _cattery_running=0
  print -n "\e]133;D;$code\e\\"
  if (( code )); then
    _cattery_report_command error
  else
    _cattery_report_command done
  fi
}

# The command line, on one line and within the 192 bytes felis keeps of a
# status title, as felis's program status (OSC 7501).
_cattery_report_command() {
  local command=${_cattery_command//[[:cntrl:]]/ }
  command=${(j: :)${=command}}
  _cattery_bytes $command
  if (( REPLY > 192 )); then
    while _cattery_bytes "$command…"; (( REPLY > 192 )); do
      command=${command[1,-2]}
    done
    command+=…
  fi
  _cattery_base64 $command
  print -n "\e]7501;state=$1:id=shell:app=cattery:title=$REPLY\e\\"
}

_cattery_bytes() {
  local LC_ALL=C
  REPLY=${#1}
}

_cattery_base64() {
  local LC_ALL=C text=$1 out= a b c n i i1 i2 i3 i4
  local table=ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/
  for (( i = 1; i <= ${#text}; i += 3 )); do
    a=${text[i]} b=${text[i+1]} c=${text[i+2]}
    n=$(( (#a & 255) << 16 ))
    [[ -n $b ]] && n=$(( n | (#b & 255) << 8 ))
    [[ -n $c ]] && n=$(( n | (#c & 255) ))
    # A subscript starting with `(` would be read as subscript flags.
    i1=$(( (n >> 18 & 63) + 1 )) i2=$(( (n >> 12 & 63) + 1 ))
    i3=$(( (n >> 6 & 63) + 1 )) i4=$(( (n & 63) + 1 ))
    out+=${table[i1]}${table[i2]}
    if [[ -n $b ]]; then out+=${table[i3]}; else out+='='; fi
    if [[ -n $c ]]; then out+=${table[i4]}; else out+='='; fi
  done
  REPLY=$out
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
