use relm4_icons_build;

fn main() {
    relm4_icons_build::bundle_icons(
        // Name of the file that will be generated at `OUT_DIR`
        "icon_names.rs",
        // Optional app ID
        Some("me.quartzy.sanicrs"),
        // Custom base resource path:
        // * defaults to `/com/example/myapp` in this case if not specified explicitly
        // * or `/org/relm4` if app ID was not specified either
        None::<&str>,
        // Directory with custom icons (if any)
        None::<&str>,
        // List of icons to include
        [
            "music-note",
            "explore2",
            "play",
            "pause",
            "next-regular",
            "previous-regular",
            "stop",
            "speaker-0",
            "speaker-1",
            "speaker-2",
            "speaker-3",
            "list",
            "playlist-consecutive",
            "playlist-repeat",
            "playlist-repeat-song",
            "playlist-shuffle"
        ],
    );
}