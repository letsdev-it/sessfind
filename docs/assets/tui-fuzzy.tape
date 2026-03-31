# VHS tape for Fuzzy search mode screenshot
# Usage: vhs docs/assets/tui-fuzzy.tape

Set Shell zsh
Set FontSize 14
Set Width 1200
Set Height 700
Set Padding 10
Set Theme "Catppuccin Mocha"
Set TypingSpeed 75ms

Hide
Type "cd ~ && ~/repos/session-seek/target/release/sessfind --mode fuzzy"
Enter
Sleep 5s
Show

Sleep 1s

Type "hello test"
Sleep 4s

# Focus results
Tab
Sleep 1s

Screenshot docs/assets/tui-fuzzy.png
