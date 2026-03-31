# VHS tape for FTS search mode screenshot
# Usage: vhs docs/assets/tui-fts.tape

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

# Focus results
Tab
Sleep 1s

Screenshot docs/assets/tui-fts.png
