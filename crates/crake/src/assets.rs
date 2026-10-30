//! Crake's own icons, bundled into the binary; everything else from gpui-kit.

/// Crake's own icons (bundled); everything else from gpui-kit.
pub(crate) struct Assets;

impl gpui_kit::AssetSource for Assets {
    fn load(&self, path: &str) -> gpui_kit::Result<Option<std::borrow::Cow<'static, [u8]>>> {
        macro_rules! icons {
            ($($name:literal),* $(,)?) => {
                match path {
                    $(concat!("crake/", $name) => {
                        return Ok(Some(std::borrow::Cow::Borrowed(include_bytes!(concat!("../../../assets/icons/", $name)))));
                    })*
                    _ => {}
                }
            };
        }
        icons!(
            "bird.svg",
            "bird-grass.svg",
            "bird-alone.svg",
            "call.svg",
            "head-warning.svg",
            "warning-triangle.svg",
            "warning-mark.svg",
            "bird-badge-warning.svg",
            "bird-badge-trouble.svg",
            "bird-badge-mark.svg",
            "bird-badge-warning-mark.svg",
            "clipboard.svg",
            "mark-routed.svg",
            "mark-left-alone.svg",
            "mark-undone.svg",
            "mark-pinned.svg",
            "person.svg",
            "shield.svg",
            "block.svg",
            "lock.svg",
            "settings.svg",
            "dots.svg",
            "circle.svg",
            "check.svg",
            "cross.svg",
            "mark-system.svg",
            "search.svg",
            "close.svg",
            "chevron-down.svg",
            "chevron-up.svg",
            "chevron-left.svg",
            "chevron-right.svg",
            "mark-gateway.svg",
            "sliders.svg",
            "grip.svg",
            "section-general.svg",
            "section-learning.svg",
            "section-names.svg",
            "section-privacy.svg",
            "section-managed.svg",
            "section-about.svg",
            "help.svg",
            "trash.svg",
            "plus.svg",
            "section-tunnels.svg",
            "disclose-right.svg",
            "disclose-down.svg",
            "broom.svg",
            "section-diagnostics.svg",
            "file.svg",
        );
        gpui_kit::assets::AllAssets.load(path)
    }

    fn list(&self, path: &str) -> gpui_kit::Result<Vec<gpui_kit::SharedString>> {
        gpui_kit::assets::AllAssets.list(path)
    }
}
