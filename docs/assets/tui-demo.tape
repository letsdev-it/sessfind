# VHS tape for sessfind demo GIF (README)
# Usage: vhs docs/assets/tui-demo.tape

Output docs/assets/tui-demo.gif

Set Shell zsh
Set FontSize 14
Set Width 1200
Set Height 700
Set Padding 10
Set Theme "Catppuccin Mocha"
Set TypingSpeed 75ms

# Start sessfind from ~ so current dir differs from session dir
Hide
Type "cd ~ && ~/repos/session-seek/target/release/sessfind"
Enter
Sleep 4s
Show

Sleep 1s

# Type the FTS search query
Type "+hell* +test +best +tool"
Sleep 3s

# Switch focus to results
Tab
Sleep 1s

# Navigate down to claude result (position 2)
Down
Sleep 1s

# Select claude session — open resume dialog
Enter
Sleep 4s

# Choose "Session directory" (first option, already selected)
Enter

# Wait for claude to load the resumed session
Sleep 10s
