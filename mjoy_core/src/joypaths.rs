use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Write};
use tracing;

#[derive(Debug, Deserialize, Serialize)]
pub struct NamedPath {
    pub full_path: String,
    pub minimal_path: String,
    pub root_event_path: String,
    pub common_name: Option<String>,
}

pub struct EventPathLookup(pub HashMap<String, String>);
impl From<Vec<NamedPath>> for EventPathLookup {
    fn from(v: Vec<NamedPath>) -> Self {
        let mut m = HashMap::new();
        for np in v {
            m.insert(np.root_event_path.clone(), np);
        }
        EventPathLookup(m)
    }
}

pub struct MinimalPathLookup(pub HashMap<String, NamedPath>);
impl From<Vec<NamedPath>> for MinimalPathLookup {
    fn from(v: Vec<NamedPath>) -> Self {
        let mut m = HashMap::new();

        for np in v {
            m.insert(np.minimal_path.clone(), np);
        }
        MinimalPathLookup(m)
    }
}

impl MinimalPathLookup {
    pub fn read_from_disk(file_path: &str) -> Result<Self, serde_json::Error> {
        let file = File::open(file_path).map_err(serde_json::Error::io)?;
        let reader = BufReader::new(file);
        let named_paths: Vec<NamedPath> = serde_json::from_reader(reader)?;
        Ok(MinimalPathLookup::from(named_paths))
    }
    pub fn write_to_disk(&self, file_path: &str) -> Result<(), serde_json::Error> {
        let file = File::create(file_path).map_err(serde_json::Error::io)?;
        let writer = BufWriter::new(file);
        let named_paths: Vec<&NamedPath> = self.0.values().collect();
        serde_json::to_writer_pretty(writer, &named_paths)?;
        Ok(())
    }
}

#[derive(Debug)]
pub enum RepathError {}

impl MinimalPathLookup {
    pub fn add_missing_paths_for_joys(
        &mut self,
        config: &crate::Config,
    ) -> Result<(), RepathError> {
        use regex::Regex;
        use std::fs;

        let paths = fs::read_dir("/dev/input/by-path")
            .expect("I really should be able to read /dev/input/by-path");

        let is_event_joy = Regex::new(r"event-joystick").expect("Compile regex");
        let path_only = Regex::new(r"/dev/input/by-path/pci.*usb.*:(.*:1)\.([0-9])-event-joystick")
            .expect("compile regex");
        let gimme_event = Regex::new(r"../event([0-9]+)").expect("compile regex");

        for path in paths {
            let path = path.expect("Path conversion failed").path();
            let full_path = path.to_str().expect("Path tostring failed");
            if is_event_joy.is_match(&full_path) {
                let partial_minimal_path = path_only
                    .captures(&full_path)
                    .unwrap()
                    .get(1)
                    .unwrap()
                    .as_str()
                    .to_string();
                let multi_controller = path_only
                    .captures(&full_path)
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

                let js_path = std::fs::read_link(&full_path)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string();

                let mut eventpath = "/dev/input/event".to_string();
                eventpath.push_str(
                    gimme_event
                        .captures(&js_path)
                        .unwrap()
                        .get(1)
                        .unwrap()
                        .as_str(),
                );

                if self.0.contains_key(&minimal_path) {
                    tracing::info!(
                        "No need to add {} ({:?}), it's already in our datastructure",
                        minimal_path,
                        self.0.get(&minimal_path).unwrap().common_name
                    );
                    continue;
                }

                let np = NamedPath {
                    full_path: full_path.to_owned(),
                    minimal_path: minimal_path.clone(),
                    root_event_path: eventpath,
                    common_name: None,
                };

                self.0.insert(minimal_path, np);
            }
        }
        Ok(())
    }
}
