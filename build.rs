use std::{env, fs::{self, Metadata}, io::Error, panic, path::Path, process::Command};

use relm4_icons_build;

// Inspired by glib-build-tools crate
pub fn compile_schemas(schemas: &[&str], target_in: Option<&str>) {
    let target = match target_in {
        Some(t) => Path::new(t).to_path_buf(),
        None => {
            let path = env::var("XDG_DATA_HOME").expect("XDG_DATA_HOME not set");
            let path = Path::new(path.as_str());
            path.join("glib-2.0/schemas")
        }
    };
    let target = target.as_path();
    if target_in.is_none() {
        fs::create_dir_all(target).expect("Error creating target directory");
    }
    if !fs::exists(target).unwrap_or(false) {
        panic!("Target directory for schema doesnt exist! ('{:?}')", target);
    }
    let metadata: Result<Metadata, Error> = fs::metadata(target);
    if let Ok(metadata) = metadata && metadata.permissions().readonly() {
        panic!("Target directory can't be written to");
    }

    for schema in schemas {
        let schema_path = Path::new(schema);
        fs::copy(schema_path, target.join(schema_path.file_name().unwrap())).expect("Error copying schema file to target dir");
    }

    let command = Command::new("glib-compile-schemas")
        // .arg("--strict")
        .arg(target)
        .output()
        .expect("Error executing glib-compile-schemas");

    if !command.status.success() {
        panic!("glib-compile-schema exited with non-zero status code {}.\nStdout: {}\nStderr: {}",
            command.status,
           String::from_utf8(command.stdout).expect("Error parsing command output"),
           String::from_utf8(command.stderr).expect("Error parsing command output"));
    }
}

fn main() {
    relm4_icons_build::bundle_icons(
        // Name of the file that will be generated at `OUT_DIR`
        "icon_names.rs",
        // Optional app ID
        Some("me.quartzy.sanicrs"),
        // Custom base resource path:
        // * defaults to `/com/example/myapp` in this case if not specified explicitly
        // * or `/org/relm4` if app ID was not specified either
        Some("/me/quartzy/sanicrs/"),
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
            "playlist-shuffle",
            "add-regular",
            // "open-menu"
        ],
    );
    compile_schemas(
        &["data/me.quartzy.sanicrs.gschema.xml"],
        None
    );
}
