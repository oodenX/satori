fn main() {
    #[cfg(feature = "gui-slint")]
    slint_build::compile("ui/overlay.slint").unwrap();
}
