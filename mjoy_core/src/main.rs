mod injoy;
mod joypaths;
mod outjoy;

use clap::Parser;
use rand;
use serde::{Deserialize, Serialize};

#[derive(Parser)]
struct Cli {
    #[clap(short, long, default_value = "config.json")]
    config: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    team_allocation: Vec<u32>,
    path_hash_salt: u32,
    team_hash_salt: u32,
    path_common_name_max_length: u32,
    hat_only_players: Vec<String>,
    number_of_multi_port_controllers_to_use: u32,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Team {
    name: String,
    players: Vec<String>,
    out_index: u32,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TeamLock {
    teams: Vec<Team>,
}

fn main() {
    let args = Cli::parse();
    let config = serde_json::from_str::<Config>(&std::fs::read_to_string(&args.config).unwrap())
        .expect("Failed to parse config file");
    dbg!(&config);
    let words = mjoy_gui::wordhash::Wordhash::new(config.path_hash_salt, config.team_hash_salt);

    // Read configuration file .json file
    let joy_paths = joypaths::repath_joys(&words, &config);
    let mpl: joypaths::MinimalPathLookup = joy_paths.unwrap().into();
    let mut minimal_paths: Vec<&String> = mpl.0.keys().collect();
    minimal_paths.sort();
    minimal_paths.reverse();

    for path in minimal_paths.iter() {
        let joy = &mpl.0[*path];
        println!("{: <15} -> {: <20}", joy.common_name, path);
    }

    let frozen_path = "teamlock.json";
    // Check for a teamlock.json file
    let frozen = if std::path::Path::new(&frozen_path).exists() {
        // If it exists, read it and return it
        let frozen =
            serde_json::from_str::<TeamLock>(&std::fs::read_to_string(&frozen_path).unwrap())
                .expect("Failed to parse frozen file");
        frozen
    } else {
        let num_required: u32 = config.team_allocation.iter().sum();
        let num_required = num_required as usize;
        if num_required != minimal_paths.len() {
            println!(
                "Incorrect number of joysticks connected. Expected {}, found {}",
                num_required,
                minimal_paths.len()
            );
            std::process::exit(1);
        }

        let mut frozen: TeamLock = TeamLock { teams: Vec::new() };
        let mut minimal_path_index = 0;
        for team_index in 0..config.team_allocation.len() {
            let mut team = Vec::new();
            for _ in 0..config.team_allocation[team_index] {
                let path = &minimal_paths[minimal_path_index];
                let joy = &mpl.0[*path];
                team.push(joy.common_name.clone());
                minimal_path_index += 1;
            }

            let mut concat = String::new();
            for player in team.iter() {
                concat.push_str(player);
                concat.push_str(".");
            }

            let team_name =
                mjoy_gui::diskteamhash::team_hash(config.team_hash_salt, concat.as_bytes());

            let team = Team {
                name: team_name,
                players: team,
                out_index: team_index as u32,
            };
            frozen.teams.push(team);
        }
        frozen
    };
    dbg!(&frozen);

    // Check frozen
    let mut total_player_count = 0;
    let mut missing_players = Vec::new();
    for team in frozen.teams.iter() {
        for player in team.players.iter() {
            let mut fail = true;
            for joy in mpl.0.values() {
                if joy.common_name == *player {
                    fail = false;
                    break;
                }
            }
            if fail {
                missing_players.push(player);
            }
            total_player_count += 1;
        }
    }

    if missing_players.len() > 0 {
        println!("Missing players:");
        for player in missing_players.iter() {
            println!("\t{}", player);
        }
        std::process::exit(1);
    }

    if total_player_count != minimal_paths.len() {
        println!(
            "Incorrect number of joysticks connected. Expected {}, found {}",
            total_player_count,
            minimal_paths.len()
        );
        std::process::exit(1);
    }

    let frozen_json = serde_json::to_string_pretty(&frozen).unwrap();
    std::fs::write(frozen_path, frozen_json).unwrap();

    use gilrs;

    let mut gilrs = gilrs::Gilrs::new().unwrap();

    // Iterate over all connected gamepads
    for (_id, gamepad) in gilrs.gamepads() {
        println!(
            "{} is {:?} {}",
            gamepad.name(),
            gamepad.power_info(),
            gamepad.devpath()
        );
    }
    let mut joy_lookup: joypaths::EventPathLookup =
        joypaths::repath_joys(&words, &config).unwrap().into();

    let feedback = {
        let mut fb = Vec::new();

        for thing in ["<", ">", "^", "v", "A", "B", "X", "Y", "L", "R", "t", "e"].iter() {
            fb.push(mjoy_gui::gui::feedback_info::ButtonPress {
                button: thing.to_string(),
                state: mjoy_gui::gui::feedback_info::PressState::Unpressed,
            });
        }
        fb
    };
    let feedback = mjoy_gui::gui::feedback_info::Presses(feedback);

    let mut fbteams = Vec::new();
    for team in frozen.teams.iter() {
        let mut fbplayers = Vec::new();
        for player in team.players.iter() {
            let fbplayer = mjoy_gui::gui::feedback_info::Player {
                player_name: player.clone(),
                feedback: feedback.clone(),
            };
            fbplayers.push(fbplayer);
        }

        let fb_team = mjoy_gui::gui::feedback_info::Team {
            team_name: &team.name,
            players: fbplayers,
            feedback: feedback.clone(),
        };

        fbteams.push(fb_team);
    }
    let mut fbinfo = mjoy_gui::gui::feedback_info::FeedbackInfo { teams: fbteams };

    let mut gui_teams = Vec::new();
    use mjoy_gui::gui::Ui;

    for team in frozen.teams.iter() {
        gui_teams.push(team.name.clone());
    }

    let mut ui = Ui::new(
        gui_teams.as_slice(),
        mjoy_gui::gui::WidthHeight::new(1920, 1080),
    );

    let all_joys = outjoy::Outjoys::new(&frozen);
    let mut thresh = 0.9f32;
    let mut change_thresh_time = std::time::Instant::now() + std::time::Duration::from_secs(1);
    let mut gui_render_time = std::time::Instant::now();
    let mut started = false;
    loop {
        let event = gilrs.next_event();

        match &event {
            Some(gilrs::Event {
                event: gilrs::EventType::Connected | gilrs::EventType::Disconnected,
                ..
            }) => {
                joy_lookup = joypaths::repath_joys(&words, &config).unwrap().into();
                continue;
            }
            _ => {}
        }

        if event.is_some() {
            continue;
        }

        all_joys.update(&mut outjoy::UpdateContext {
            gilrs: &mut gilrs,
            event_path_lookup: &joy_lookup,
            feedback: &mut fbinfo,
            stick_only_names: &config.hat_only_players,
            button_threshold: thresh,
        });

        if std::time::Instant::now()
            .checked_duration_since(gui_render_time)
            .is_some()
        {
            gui_render_time = std::time::Instant::now() + std::time::Duration::from_millis(50);
            ui.render(&fbinfo, started);

            if !started {
                let mut should_start = true;
                for (i, joystick) in gilrs.gamepads() {
                    let devpath = joystick.devpath();
                    let common_name = match &joy_lookup.0.get(devpath) {
                        Some(name) => &name.common_name,
                        None => {
                            continue;
                        }
                    };
                    for team in frozen.teams.iter() {
                        if !should_start {
                            break;
                        }

                        for player in team.players.iter() {
                            if player == common_name {
                                match joystick.button_data(gilrs::Button::West) {
                                    Some(btn) => {
                                        if !btn.is_pressed() {
                                            should_start = false;
                                        }
                                    }
                                    None => {
                                        should_start = false;
                                    }
                                }
                                match joystick.button_data(gilrs::Button::DPadRight) {
                                    Some(btn) => {
                                        if !(btn.value() < 0.1f32) {
                                            should_start = false;
                                        }
                                    }
                                    None => {
                                        should_start = false;
                                    }
                                }
                            }
                        }
                    }
                }

                if should_start {
                    started = true;
                }
            }
        } else {
            continue;
        }

        let now = std::time::Instant::now();
        if now.checked_duration_since(change_thresh_time).is_some() {
            change_thresh_time = change_thresh_time + {
                // Random number up to 5000
                let random_millis = rand::random::<u64>() % 5000;
                let random_millis = random_millis + 300;
                std::time::Duration::from_millis(random_millis)
            };
            thresh = {
                let rand = rand::random::<u64>();
                let rand = rand % 10000;
                let rand = rand as f32;
                let rand = rand / 10000.0;
                let mut rand = rand * 0.61;
                rand += 0.49;
                rand.min(0.95f32)
            };
        }

        //if let Some(gilrs::Event { id, event, time }) = event {
        //let gp = gilrs.gamepad(id);
        //let devpath = gp.devpath();
        //dbg!(&devpath);
        //let path = joy_lookup.0.get(devpath);
        //if path.is_none() {
        //println!("Unknown device: {:?}", devpath);
        //continue;
        //}
        //let common_name = path.unwrap().common_name.clone();
        //println!("{:?} New event from {}: {:?}", time, common_name, event);

        //dbg!(gp.button_data(gilrs::Button::DPadRight));
        //}
    }
}

////Exit with code 1
//std::process::exit(1);
//

//let input_joysticks_vector_of_vectors = {
//let mut input_vec_of_vecs = vec![];
//for _ in 0..number_of_output_controllers {
//input_vec_of_vecs.push(Vec::new())
//}
//let mut output_joystick_index = 0;
//let mut num_assigned_to_current_output_index = 0;
//let mut mayflash_count = 0;
//for i in 0..num_inputs_detected {
//if sdl2_input_joystick_subsystem
//.name_for_index(i)?
//.contains("MAYFLASH")
//{
//if mayflash_count >= args.mayflash_controllers {
//continue;
//} else {
//mayflash_count += 1;
//}
//}
//let current_in_joy = sdl2_input_joystick_subsystem.open(i)?;
//let data = current_in_joy.guid().raw().data;

//// Calculate sha256 for the data
//let mut hasher = sha2::Sha256::new();
//hasher.update(data);
//let by = current_in_joy.instance_id().to_le_bytes();
//hasher.update(by);
//let result = hasher.finalize();
//println!("GUID: {}", current_in_joy.guid());
//let num = result[0] as u16;
//let num = num << 3;
//let num = num | ((result[1] & 0b00000111) as u16);
//println!("myname: {}", english_words[num as usize]);

//input_vec_of_vecs[output_joystick_index].push(current_in_joy);
//num_assigned_to_current_output_index += 1;

//// Test: should we move on and start assigning joysticks for the next output contorller?
//if {
//let currently_assigning_last_output =
//output_joystick_index >= (number_of_output_controllers - 1) as usize;
//let minimim_joysticks_per_output_controller = ((num_inputs_detected as f32)
//// (number_of_output_controllers as f32))
//.floor() as i32;
//num_assigned_to_current_output_index >= minimim_joysticks_per_output_controller
//&& !currently_assigning_last_output
//} {
//output_joystick_index += 1;
//num_assigned_to_current_output_index = 0;
//}
//}
//input_vec_of_vecs
//};

//let out_joysticks = {
//let mut out_joysticks = Vec::new();
//for i in 0..number_of_output_controllers {
//let name = format!("Buster{}", i);
//let outjoy = software_joystick::Joystick::new(name)?;
//out_joysticks.push(outjoy);
//}
//out_joysticks
//};

//loop {
//sdl2_input_joystick_subsystem.update();
//for curr_out_joy_index in 0..out_joysticks.len() {
//let out_joystick = &out_joysticks[curr_out_joy_index];
//let input_joystick_vector = &input_joysticks_vector_of_vectors[curr_out_joy_index];

//// Iterates over all of the axes we care about- each element of the NamedAxis enum
//for named_axis in NamedAxis::iter() {
//let (id, scalar_map) = snes_namedaxis_to_id_and_scalar(&named_axis);

//out_joystick.move_axis(
//{
//match named_axis {
//NamedAxis::Xright => software_joystick::Axis::X,
//NamedAxis::Yup => software_joystick::Axis::Y,
//}
//},
//{
//let sum: f32 = input_joystick_vector
//.iter()
//.map(|ijoy| {
//let raw_axis_value = ijoy.axis(id).unwrap() as f32;
//let normalized_axis_value = raw_axis_value / scalar_map;
//normalized_axis_value
//})
//.sum();
//let average: f32 = sum / (input_joystick_vector.len() as f32);
//let exponent_scaled =
//average.signum() * average.abs().powf(args.summed_axis_exponent);
//let out_scaled = exponent_scaled * 512.0;
//out_scaled.trunc() as i32
//},
//)?;
//}

//// Iterates over all the buttons we care about
//for named_button in NamedButton::iter() {
//out_joystick.button_press(
//match named_button {
//NamedButton::X => software_joystick::Button::RightNorth,
//NamedButton::A => software_joystick::Button::RightEast,
//NamedButton::B => software_joystick::Button::RightSouth,
//NamedButton::Y => software_joystick::Button::RightWest,
//NamedButton::L => software_joystick::Button::L1,
//NamedButton::R => software_joystick::Button::R1,
//NamedButton::Start => software_joystick::Button::RightSpecial,
//NamedButton::Select => software_joystick::Button::LeftSpecial,
//},
//{
//let sdl_inputs_button_id = snes_namedbutton_to_id(&named_button);
//let in_joys_to_check = {
//if args.p1_controller1_buttons_always_down
//&& curr_out_joy_index == 0
//&& input_joystick_vector.len() > 1
//{
//&input_joystick_vector[1..]
//} else {
//&input_joystick_vector
//}
//};
//let is_pressed = in_joys_to_check
//.iter()
//.all(|ijoy| ijoy.button(sdl_inputs_button_id).unwrap());
//is_pressed
//},
//)?;
//}
//out_joystick.synchronise()?;
//}
//}

// loop {
//     joystick_subsystem.update();
//     for (js_index, js) in joy_vec.iter().enumerate() {
//         for button_index in 0..js.num_buttons() {
//             if js.button(button_index)? {
//                 println!("Pressed! Controller {}, button {}", js_index, button_index);
//             }
//         }
//     }
// }

// // let js = joystick_subsystem.open(0)?;
// return Ok(());

// let joystick = joystick::Joystick::new("BustersDirtySecret".into())?;

// println!(
//     "Created joystick with device path {}",
//     joystick.device_path()?.to_string_lossy()
// );

// loop {
//     joystick.button_press(joystick::Button::LeftNorth, true)?;
//     joystick.button_press(joystick::Button::RightSouth, true)?;
//     joystick.move_axis(joystick::Axis::Y, 100)?;

//     joystick.synchronise()?;
// }
//}

// fn button_map(i: usize) -> joystick::Button {
//     use joystick::Button::*;
//     match i {
//         0 => LeftNorth,
//         1 => LeftWest,
//         2 => LeftEast,
//         3 => LeftSouth,
//         4 => LeftSpecial,
//         5 => RightSouth,
//         6 => RightSpecial,
//         7 => RightEast,
//         8 => RightWest,
//         9 => RightNorth,
//         10 => R2,
//         11 => R1,
//         12 => L2,
//         13 => L1,
//         _ => unreachable!(),
//     }
// }

// fn axis_map(i: usize) -> joystick::Axis {
//     use joystick::Axis::*;
//     match i {
//         0 => X,
//         1 => Y,
//         2 => RX,
//         3 => RY,
//         _ => unreachable!(),
//     }
// }
