use mjoy_gui::wordhash::Wordhash;
use std::collections::HashMap;

#[derive(Debug)]
pub struct NamedPath {
    pub full_path: String,
    pub minimal_path: String,
    pub root_event_path: String,
    pub common_name: String,
}

pub struct EventPathLookup(pub HashMap<String, NamedPath>);
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

#[derive(Debug)]
pub enum RepathError {}

pub fn repath_joys(
    words: &Wordhash,
    config: &crate::Config,
) -> Result<Vec<NamedPath>, RepathError> {
    use regex::Regex;
    use std::fs;

    let mut joy_paths = Vec::new();

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
            let mut minimal_path_bytes = minimal_path.as_bytes().to_vec();

            let common_name =
                words.object_name(&mut minimal_path_bytes, config.path_common_name_max_length);

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

            joy_paths.push(NamedPath {
                full_path: full_path.to_owned(),
                minimal_path,
                root_event_path: eventpath,
                common_name: common_name.to_owned(),
            });
        }
    }

    Ok(joy_paths)
}
