use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, BufWriter, Write};
use tracing;

#[derive(Debug, Deserialize, Serialize)]
pub struct NamedPath {
    pub full_path: String,
    pub minimal_path: String,
    pub root_event_path: String,
    pub common_name: Option<String>,
}

#[derive(Debug)]
pub struct EventPathLookup(pub HashMap<String, String>);

#[derive(Debug)]
pub struct MinimalPathLookup(pub HashMap<String, NamedPath>);

pub fn repath(config: &crate::Config) -> Vec<NamedPath> {
    let paths = fs::read_dir("/dev/input/by-path").expect("Failed to read /dev/input/by-path");
    let is_event_joy = Regex::new(r"event-joystick").expect("Failed to compile regex");
    let path_only = Regex::new(r"/dev/input/by-path/pci.*usb.*:(.*:1)\.([0-9])-event-joystick")
        .expect("Failed to compile regex");
    let gimme_event = Regex::new(r"../event([0-9]+)").expect("Failed to compile regex");

    let mut discovered_paths = Vec::new();

    for path in paths {
        let path = path.expect("Path conversion failed").path();
        let full_path = path.to_str().expect("Path to string failed");

        if is_event_joy.is_match(full_path) {
            let partial_minimal_path = path_only
                .captures(full_path)
                .unwrap()
                .get(1)
                .unwrap()
                .as_str()
                .to_string();
            let multi_controller = path_only
                .captures(full_path)
                .unwrap()
                .get(2)
                .unwrap()
                .as_str()
                .to_string();

            if config.number_of_multi_port_controllers_to_use
                <= multi_controller.parse::<u32>().unwrap()
            {
                continue;
            }

            let minimal_path = format!("{}.{}", partial_minimal_path, multi_controller);
            let js_path = fs::read_link(full_path)
                .unwrap()
                .to_str()
                .unwrap()
                .to_string();
            let eventpath = format!(
                "/dev/input/event{}",
                gimme_event
                    .captures(&js_path)
                    .unwrap()
                    .get(1)
                    .unwrap()
                    .as_str()
            );

            discovered_paths.push(NamedPath {
                full_path: full_path.to_string(),
                minimal_path: minimal_path.clone(),
                root_event_path: eventpath,
                common_name: None,
            });
        }
    }

    discovered_paths
}

impl MinimalPathLookup {
    pub fn read_from_disk(file_path: &str) -> Self {
        match File::open(file_path) {
            Ok(file) => {
                let reader = BufReader::new(file);
                // Specify that the deserialized type should be Vec<NamedPath>
                match serde_json::from_reader::<_, Vec<NamedPath>>(reader) {
                    Ok(named_paths) => {
                        let lookup = named_paths
                            .into_iter()
                            .map(|np| (np.minimal_path.clone(), np))
                            .collect();
                        MinimalPathLookup(lookup)
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse bindings: {}", e);
                        MinimalPathLookup(HashMap::new())
                    }
                }
            }
            Err(_) => {
                tracing::warn!(
                    "No bindings file found at '{}', using empty structure.",
                    file_path
                );
                MinimalPathLookup(HashMap::new())
            }
        }
    }

    pub fn write_to_disk(&self, file_path: &str) -> Result<(), serde_json::Error> {
        let file = File::create(file_path).map_err(serde_json::Error::io)?;
        let writer = BufWriter::new(file);
        let named_paths: Vec<&NamedPath> = self.0.values().collect();
        serde_json::to_writer_pretty(writer, &named_paths)
    }

    pub fn add_missing_paths_for_joys(&mut self, config: &crate::Config) {
        let discovered_paths = repath(config);

        // Add new paths from discovery if they don't already exist
        for np in discovered_paths {
            self.0.entry(np.minimal_path.clone()).or_insert(np);
        }
    }
}

impl EventPathLookup {
    pub fn repath(config: &crate::Config) -> Self {
        let discovered_paths = repath(config);
        let mut lookup = EventPathLookup(HashMap::new());

        for np in discovered_paths {
            lookup.0.insert(np.root_event_path.clone(), np.minimal_path);
        }

        lookup
    }
}
