fn main() {
    // The builder script passes the owner's first name in; it becomes the
    // default masthead ("<Name>'s Daily") until they change it in the app.
    println!("cargo:rerun-if-env-changed=DAILY_OWNER_NAME");
    tauri_build::build()
}
