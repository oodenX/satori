fn main() {
    #[cfg(feature = "gui-slint")]
    slint_build::compile("src/overlay.slint").unwrap();
}
