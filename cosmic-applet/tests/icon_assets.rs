//! # Icon asset tests — every sensor icon the panel looks up must ship in this crate.
//!
//! Panel icons are resolved by freedesktop icon name at render time, so a name that no
//! installed SVG backs is not an error: it silently draws nothing. That is exactly how the
//! bug these tests guard against presented — the panel borrowed minimon-applet's icon IDs,
//! so the icons resolved fine while minimon was installed and vanished when it was removed.

use cosmic_applet::ui::panel_widget::OWN_SENSOR_ICONS;
use std::path::Path;

fn icons_dir() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/res/icons"))
}

/// Each icon ID we render must have a same-named SVG in res/icons/, because that is the
/// directory every installer in this repo copies onto the icon theme path.
#[test]
fn shipped_sensor_icons_exist() {
    let dir = icons_dir();
    for id in OWN_SENSOR_ICONS {
        let svg = dir.join(format!("{id}.svg"));
        assert!(
            svg.is_file(),
            "panel looks up icon '{id}' but {} is not shipped",
            svg.display()
        );
    }
}

/// A zero-byte or non-SVG file still satisfies `is_file()`, so check the payload too.
#[test]
fn shipped_sensor_icons_are_non_empty_svgs() {
    for id in OWN_SENSOR_ICONS {
        let svg = icons_dir().join(format!("{id}.svg"));
        let body = std::fs::read_to_string(&svg)
            .unwrap_or_else(|e| panic!("could not read {}: {e}", svg.display()));
        assert!(body.contains("<svg"), "{id}: file has no <svg element");
        assert!(
            body.contains("<path"),
            "{id}: file has no drawable <path>, would render blank"
        );
    }
}

/// The regression itself: this applet must not name icons belonging to another applet.
/// minimon's IDs live in its package namespace and are only on the lookup path while its
/// package is installed, so referencing them makes our panel depend on a foreign package.
#[test]
fn panel_does_not_reference_foreign_applet_icons() {
    let src = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/ui/panel_widget.rs"
    ));
    let code = std::fs::read_to_string(src).expect("could not read panel_widget.rs");
    assert!(
        !code.contains("io.github.cosmic_utils"),
        "panel_widget.rs references another applet's icon IDs; vendor the SVG into \
         res/icons/ and use the com.cosmic.network_system_monitor-<sensor> name instead"
    );
}

/// Resolve every sensor icon through libcosmic's own finder — the same code the panel runs
/// (`widget::icon::Named::path`) — fed res/icons/ as a flat extra search path. This proves
/// the shipped names actually resolve, rather than only proving a file is present.
///
/// It deliberately does not depend on what is installed on the test machine: extra_paths is
/// searched before the icon theme chain, so no system theme is consulted.
#[test]
fn sensor_icons_resolve_through_the_cosmic_icon_finder() {
    let dir = icons_dir().to_path_buf();
    for id in OWN_SENSOR_ICONS {
        let found = cosmic::widget::icon::Named::new(*id)
            .with_extra_paths(vec![dir.clone()])
            .path();
        match found {
            Some(path) => {
                assert_eq!(
                    path.file_stem().and_then(std::ffi::OsStr::to_str),
                    Some(*id),
                    "'{id}' resolved to a different icon ({}) — name truncation fallback \
                     is silently substituting another asset",
                    path.display()
                );
                assert_eq!(
                    path.extension().and_then(std::ffi::OsStr::to_str),
                    Some("svg")
                );
            }
            // The theme chain is not consulted for the app icon in this test, so a miss here
            // means the name genuinely does not match any shipped file.
            None => panic!("'{id}' did not resolve to any icon under {}", dir.display()),
        }
    }
}
