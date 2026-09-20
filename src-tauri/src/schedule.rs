//! Morning delivery: a macOS LaunchAgent that runs this same app with
//! `--refresh-only` once a day, so the edition is already printed and cached
//! when the window opens.
//!
//! The app installs and removes the agent itself (the switch in the masthead),
//! so there is nothing to paste into a terminal:
//!   ~/Library/LaunchAgents/com.mydailynewspaper.refresh.plist
//! If the Mac is asleep at the scheduled time, launchd runs the job on wake.
//! The plist is the single source of truth for whether delivery is on and when.

use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

use regex::Regex;
use serde::Serialize;
use tauri::AppHandle;

use crate::store;

pub const LABEL: &str = "com.mydailynewspaper.refresh";
/// What the job was called when this app was "Richard's Daily".
const LEGACY_LABEL: &str = "com.richardgarza.daily.refresh";
pub const REFRESH_FLAG: &str = "--refresh-only";

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleInfo {
    /// False anywhere but macOS.
    pub supported: bool,
    pub enabled: bool,
    pub hour: u32,
    pub minute: u32,
}

fn supported() -> bool {
    cfg!(target_os = "macos")
}

fn agent_path(label: &str) -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join("Library/LaunchAgents").join(format!("{label}.plist")))
}

fn plist_path() -> Option<PathBuf> {
    agent_path(LABEL)
}

/// Carry a schedule over from the old job name: same time, new label, and the
/// old job removed so the paper isn't delivered twice.
pub fn migrate_legacy(app: &AppHandle) {
    if !supported() {
        return;
    }
    let Some(old) = agent_path(LEGACY_LABEL) else { return };
    let Ok(text) = std::fs::read_to_string(&old) else { return };
    if let Ok(uid) = current_uid() {
        let _ = Command::new("launchctl").args(["bootout", &format!("gui/{uid}/{LEGACY_LABEL}")]).output();
    }
    let _ = std::fs::remove_file(&old);
    if !status().enabled {
        if let Some((h, m)) = parse_time(&text) {
            let _ = set(app, true, h, m);
        }
    }
}

pub fn status() -> ScheduleInfo {
    let mut info = ScheduleInfo { supported: supported(), enabled: false, hour: 6, minute: 0 };
    if !info.supported {
        return info;
    }
    if let Some(text) = plist_path().and_then(|p| std::fs::read_to_string(p).ok()) {
        if let Some((h, m)) = parse_time(&text) {
            info.enabled = true;
            info.hour = h;
            info.minute = m;
        }
    }
    info
}

pub fn set(app: &AppHandle, enabled: bool, hour: u32, minute: u32) -> Result<ScheduleInfo, String> {
    if !supported() {
        return Err("Morning delivery uses macOS's scheduler, so it only works on a Mac.".into());
    }
    let path = plist_path().ok_or("Can't find your home folder.")?;
    let uid = current_uid()?;
    let domain = format!("gui/{uid}");

    // Always unload first: bootstrap refuses to replace a loaded job.
    let _ = Command::new("launchctl").args(["bootout", &format!("{domain}/{LABEL}")]).output();

    if !enabled {
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| format!("Couldn't remove {}: {e}", path.display()))?;
        }
        return Ok(status());
    }

    let exe = std::env::current_exe().map_err(|e| format!("Can't tell where the app is installed: {e}"))?;
    let log = store::data_dir(app)?.join("background.log");
    let text = plist(&exe.to_string_lossy(), &log.to_string_lossy(), hour.min(23), minute.min(59));

    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("Couldn't create {}: {e}", dir.display()))?;
    }
    std::fs::write(&path, text).map_err(|e| format!("Couldn't write {}: {e}", path.display()))?;

    let path_str = path.to_string_lossy().to_string();
    let boot = Command::new("launchctl")
        .args(["bootstrap", &domain, &path_str])
        .output()
        .map_err(|e| format!("Couldn't run launchctl: {e}"))?;
    if !boot.status.success() {
        // Older macOS spelling.
        let legacy = Command::new("launchctl").args(["load", "-w", &path_str]).output();
        let legacy_ok = legacy.as_ref().map(|o| o.status.success()).unwrap_or(false);
        if !legacy_ok {
            let err = String::from_utf8_lossy(&boot.stderr).trim().to_string();
            let _ = std::fs::remove_file(&path);
            return Err(format!("macOS wouldn't register the schedule: {err}"));
        }
    }
    Ok(status())
}

fn current_uid() -> Result<String, String> {
    let out = Command::new("id").arg("-u").output().map_err(|e| format!("Couldn't run id: {e}"))?;
    let uid = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if uid.is_empty() || !uid.chars().all(|c| c.is_ascii_digit()) {
        return Err("Couldn't work out your user id.".into());
    }
    Ok(uid)
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

pub fn plist(exe: &str, log: &str, hour: u32, minute: u32) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{label}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{exe}</string>
    <string>{flag}</string>
  </array>
  <key>StartCalendarInterval</key>
  <dict>
    <key>Hour</key>
    <integer>{hour}</integer>
    <key>Minute</key>
    <integer>{minute}</integer>
  </dict>
  <key>RunAtLoad</key>
  <false/>
  <key>StandardOutPath</key>
  <string>{log}</string>
  <key>StandardErrorPath</key>
  <string>{log}</string>
</dict>
</plist>
"#,
        label = LABEL,
        exe = xml_escape(exe),
        flag = REFRESH_FLAG,
        log = xml_escape(log),
    )
}

pub fn parse_time(plist_text: &str) -> Option<(u32, u32)> {
    static HOUR: OnceLock<Regex> = OnceLock::new();
    static MINUTE: OnceLock<Regex> = OnceLock::new();
    let hour = HOUR.get_or_init(|| Regex::new(r"<key>Hour</key>\s*<integer>(\d{1,2})</integer>").unwrap());
    let minute = MINUTE.get_or_init(|| Regex::new(r"<key>Minute</key>\s*<integer>(\d{1,2})</integer>").unwrap());
    let h: u32 = hour.captures(plist_text)?[1].parse().ok()?;
    let m: u32 = minute.captures(plist_text).and_then(|c| c[1].parse().ok()).unwrap_or(0);
    (h < 24 && m < 60).then_some((h, m))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plist_round_trips_and_escapes() {
        let text = plist("/Applications/My Daily Newspaper.app/Contents/MacOS/my-daily-newspaper", "/Users/r&d/Library/x.log", 6, 30);
        assert_eq!(parse_time(&text), Some((6, 30)));
        assert!(text.contains("<string>/Applications/My Daily Newspaper.app/Contents/MacOS/my-daily-newspaper</string>"));
        assert!(text.contains("<string>--refresh-only</string>"));
        assert!(text.contains("/Users/r&amp;d/Library/x.log"));
        assert!(!text.contains("r&d"));
    }

    #[test]
    fn rejects_nonsense_times() {
        assert_eq!(parse_time("<key>Hour</key><integer>25</integer>"), None);
        assert_eq!(parse_time("no schedule here"), None);
        assert_eq!(parse_time("<key>Hour</key>\n <integer>7</integer>"), Some((7, 0)));
    }
}
