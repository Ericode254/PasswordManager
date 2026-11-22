# Record with:  vhs demo/demo.tape   (run from the repo root, after demo/setup.sh)
# Uses only the throwaway sandbox from demo/env.sh; every secret shown is fake.
Output demo/passtui.gif

Set Shell "bash"
Set FontSize 17
Set Width 1200
Set Height 700
Set Padding 16
Set Theme "Catppuccin Mocha"
Set TypingSpeed 60ms
Set Framerate 24

Hide
# Reset state from earlier takes, so every recording starts the same.
Type "source demo/env.sh && pass rm -f netflix.com/home >/dev/null 2>&1; rm -rf $XDG_DATA_HOME; alias passtui=./target/release/passtui; clear" Enter
Sleep 500ms
Show

# 1. Unlock with the master password
Type "passtui" Sleep 300ms Enter
Sleep 1.5s
Type "demo-master-pass-2026" Sleep 300ms Enter
Sleep 1.5s

# 2. Fuzzy search and open an entry
Type "/" Sleep 300ms
Type "gitwork" Sleep 700ms
Enter Sleep 400ms
Enter Sleep 1.5s

# 3. Reveal the password, copy it
Type "p" Sleep 2s
Type "p" Sleep 500ms
Type "y" Sleep 1.5s

# 4. TOTP code
Type "t" Sleep 2.5s
Escape Sleep 500ms

# 5. Recovery codes
Type "R" Sleep 2.5s
Escape Sleep 500ms

# 6. Favourite it
Type "f" Sleep 1.2s

# 7. Back to the full list, then add an entry with a generated password
Escape Sleep 400ms
Escape Sleep 600ms
Type "a" Sleep 800ms
Type "netflix.com/home" Sleep 300ms
Tab Sleep 300ms
Ctrl+G Sleep 1.5s
Enter Sleep 500ms
Tab Sleep 300ms
Type "demo@example.com" Sleep 300ms
Enter Sleep 1.5s

# 8. Store history, including a deleted entry
Type "H" Sleep 3s
Escape Sleep 500ms

# 9. Help, then lock
Type "?" Sleep 3s
Escape Sleep 400ms
Ctrl+L Sleep 2s
Type "q" Sleep 600ms

# 10. Quick picker: search, copy, exit
Type "passtui --pick" Sleep 300ms Enter
Sleep 1.2s
Type "demo-master-pass-2026" Sleep 300ms Enter
Sleep 1s
Type "netl" Sleep 1s
Enter Sleep 1.5s
