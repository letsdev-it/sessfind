# VHS tape for resume dialog screenshot
# Usage: vhs docs/assets/tui-resume.tape

Set Shell zsh
Set FontSize 14
Set Width 1200
Set Height 700
Set Padding 10
Set Theme "Catppuccin Mocha"

Hide
Type "cd ~ && ~/repos/session-seek/target/release/sessfind"
Enter
Sleep 4s
Show

Type "+hell* +test +best +tool"
Sleep 3s

# Focus results and go to position 3 (codex)
Tab
Sleep 1s
Down
Sleep 500ms
Down
Sleep 1s

# Open resume dialog
Enter
Sleep 3s

Screenshot docs/assets/tui-resume.png
