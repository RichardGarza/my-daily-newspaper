#!/bin/bash
# Double-click me. I build "My Daily Newspaper.app" for YOU - your name on the
# masthead, your initials on the icon - put it in Applications and open it.
# After that you never need this file (or a terminal) again.
#
# What I do, in order:
#   1. make sure Apple's command line tools are there
#   2. install Rust if it's missing (one time, ~5 min, goes in ~/.cargo)
#   3. npm install
#   4. ask your first name (once) and draw your icon
#   5. compile the app (first time 3-6 minutes)
#   6. copy it to /Applications and launch it
#
# Nothing here touches your Claude or Grok logins. The app talks to the
# `claude` command that is already signed in on this Mac.

# Work from the project folder, even if this file was copied somewhere else
# (the Desktop, say).
cd "$(dirname "$0")" || exit 1
for guess in "$HOME/projects/my-daily-newspaper" "$HOME/my-daily-newspaper" "$HOME/projects/richards-daily"; do
  [ -f package.json ] && [ -d src-tauri ] && break
  cd "$guess" 2>/dev/null
done
if [ ! -f package.json ] || [ ! -d src-tauri ]; then
  echo "Can't find the my-daily-newspaper project folder. Run this file from inside it."
  read -n 1 -s -r -p "Press any key to close."
  exit 1
fi

# Keep a copy of everything below in build.log, next to this file. If a build
# fails, that file has the details.
exec > >(tee "build.log") 2>&1
echo "Build started $(date)"

say_step() { printf "\n\033[1m== %s\033[0m\n" "$1"; }
fail() {
  printf "\n\033[31mStopped: %s\033[0m\n" "$1"
  printf "The details are saved in build.log, in the project folder.\n\n"
  read -n 1 -s -r -p "Press any key to close."
  exit 1
}

# A double-clicked script doesn't get your terminal's PATH. Borrow it.
SHELL_PATH="$(/bin/zsh -lic 'printf "__P__%s\n" "$PATH"' 2>/dev/null | sed -n 's/^__P__//p' | tail -1)"
[ -n "$SHELL_PATH" ] && export PATH="$SHELL_PATH"
export PATH="$HOME/.cargo/bin:$PATH:/opt/homebrew/bin:/usr/local/bin:$HOME/.homebrew/bin:$HOME/.local/bin"

say_step "1/6  Apple command line tools"
if ! xcode-select -p >/dev/null 2>&1; then
  xcode-select --install
  fail "Apple's command line tools are installing (a window just opened). When it finishes, double-click this file again."
fi
# Will the compiler actually run? After an Xcode update it refuses to until the
# license is accepted again - and then every Rust crate fails in a wall of red.
cc_check() { echo 'int main(void){return 0;}' | cc -x c - -o "${TMPDIR:-/tmp}/mdn-cc-check" 2>&1; }
CC_OUT="$(cc_check)"
if [ $? -ne 0 ]; then
  if echo "$CC_OUT" | grep -qi "license"; then
    echo "Xcode was updated and won't compile anything until its license is accepted again."
    echo "Apple's own prompt comes next: enter your Mac password (nothing shows while you"
    echo "type), press space to page through it (q jumps to the end), then type: agree"
    echo
    sudo xcodebuild -license || fail "The Xcode license wasn't accepted, so nothing can be compiled yet."
    CC_OUT="$(cc_check)" || fail "The compiler still won't run: $CC_OUT"
  else
    fail "The C compiler won't run: $CC_OUT"
  fi
fi
echo "ok"

say_step "2/6  Rust"
if ! command -v cargo >/dev/null 2>&1; then
  echo "Not found - installing (one time)..."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y || fail "Rust install failed."
  # shellcheck disable=SC1091
  source "$HOME/.cargo/env"
fi
cargo --version || fail "Rust still isn't on the PATH."

say_step "3/6  JavaScript packages"
command -v npm >/dev/null 2>&1 || fail "npm not found. Install Node.js 20 or newer (https://nodejs.org) and run me again."
NODE_MAJOR="$(node -p 'process.versions.node.split(".")[0]' 2>/dev/null)"
[ "${NODE_MAJOR:-0}" -ge 18 ] || fail "Node.js $(node -v) is too old. Install Node.js 20 or newer (https://nodejs.org) and run me again."
npm install --no-audit --no-fund || fail "npm install failed."
# npm sometimes skips the Mac-specific native pieces of the build tools.
# If the bundler can't start, wipe and reinstall once from scratch.
if ! npx --no-install vite --version >/dev/null 2>&1; then
  echo "Build tools didn't install cleanly - reinstalling from scratch..."
  rm -rf node_modules package-lock.json
  npm install --no-audit --no-fund || fail "npm install failed (second try)."
  npx --no-install vite --version >/dev/null 2>&1 || fail "The bundler (vite) still won't start. Node version: $(node -v)"
fi
echo "node $(node -v) · vite $(npx --no-install vite --version 2>/dev/null)"

say_step "4/6  Whose paper is this?"
# Remembered in .owner (never committed). Delete that file to be asked again,
# or pass a name:  ./Build\ My\ Daily\ Newspaper.command Priya
clean_name() { printf '%s' "$1" | tr -d '\000-\037"\\<>&' | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//' | cut -c1-40; }
OWNER="$(clean_name "$1")"
[ -z "$OWNER" ] && [ -f .owner ] && OWNER="$(clean_name "$(head -n 1 .owner)")"
if [ -z "$OWNER" ]; then
  echo "Your first name goes on the masthead (\"Sam's Daily\") and your initial on the icon."
  read -r -p "Your first name (or just Enter to skip): " TYPED </dev/tty
  OWNER="$(clean_name "$TYPED")"
fi
if [ -n "$OWNER" ]; then
  printf '%s\n' "$OWNER" > .owner
  echo "Building ${OWNER}'s Daily."
else
  echo "No name given - the masthead will say \"My Daily\" until you set one in the app."
fi
export DAILY_OWNER_NAME="$OWNER"

ICON_DIR="src-tauri/icons-personal"
LETTERS="$(cargo run --quiet --release --manifest-path tools/icon-maker/Cargo.toml -- "$OWNER" "$ICON_DIR")" \
  || fail "Couldn't draw the icon."
echo "Icon: $LETTERS"
ICON_CONFIG='{"bundle":{"icon":["icons-personal/32x32.png","icons-personal/128x128.png","icons-personal/128x128@2x.png","icons-personal/icon.icns","icons-personal/icon.ico","icons-personal/icon.png"]}}'

say_step "5/6  Compiling My Daily Newspaper (first time: 3-6 minutes)"
npm run tauri build -- --bundles app --config "$ICON_CONFIG" || fail "The build failed."

APP="src-tauri/target/release/bundle/macos/My Daily Newspaper.app"
[ -d "$APP" ] || fail "Build finished but I can't find the app at: $APP"

say_step "6/6  Installing"
DEST="/Applications"
[ -w "$DEST" ] || { DEST="$HOME/Applications"; mkdir -p "$DEST"; }
osascript -e 'tell application "My Daily Newspaper" to quit' >/dev/null 2>&1
rm -rf "$DEST/My Daily Newspaper.app"
cp -R "$APP" "$DEST/" || fail "Couldn't copy the app into $DEST."
touch "$DEST/My Daily Newspaper.app"   # nudges Finder and the Dock to pick up a new icon

# This app used to be called "Richards Daily". The new one picks up the old
# one's interests, editions and delivery schedule on first launch, so the old
# copy only causes confusion.
for OLD in "/Applications/Richards Daily.app" "$HOME/Applications/Richards Daily.app"; do
  if [ -d "$OLD" ]; then
    osascript -e 'tell application "Richards Daily" to quit' >/dev/null 2>&1
    rm -rf "$OLD" && echo "Removed the old copy: $OLD"
  fi
done

printf "\n\033[1mDone.\033[0m  %s/My Daily Newspaper.app\n" "$DEST"
command -v claude >/dev/null 2>&1 || printf "\033[33mHeads up:\033[0m the \`claude\` command isn't on this Mac yet. Install Claude Code and sign in, or the presses can't run: https://claude.com/claude-code\n"
echo "Drag it to your Dock if you want it there. Opening it now..."
open "$DEST/My Daily Newspaper.app"
echo
read -n 1 -s -r -p "Press any key to close."
