# Cattery starts zsh with ZDOTDIR pointing here, to load its integration.
# The user's ZDOTDIR goes back first, so zsh reads the rest of their startup
# files from where it always does.
if (( ${+CATTERY_ZDOTDIR} )); then
  ZDOTDIR=$CATTERY_ZDOTDIR
  unset CATTERY_ZDOTDIR
else
  unset ZDOTDIR
fi
[[ -f ${ZDOTDIR:-$HOME}/.zshenv ]] && builtin source ${ZDOTDIR:-$HOME}/.zshenv

if [[ -o interactive && -n $CATTERY_SHELL_INTEGRATION ]]; then
  # Loaded at the first prompt rather than now: its hooks must run after
  # those the prompt theme in .zshrc adds, which rebuild PS1.
  autoload -Uz add-zsh-hook
  _cattery_integrate() {
    add-zsh-hook -d precmd _cattery_integrate
    unfunction _cattery_integrate
    builtin source $CATTERY_SHELL_INTEGRATION
    _cattery_mark_prompt
  }
  add-zsh-hook precmd _cattery_integrate
fi
