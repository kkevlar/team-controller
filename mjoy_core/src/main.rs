mod bindings;
mod injoy;
mod joypaths;
mod outjoy;
mod team_select;

use clap::Parser;
use mjoy_gui::gui::feedback_info::FeedbackInfo;
use rand;
use serde::{Deserialize, Serialize};
use std::cell::{self, RefCell};
use std::collections::HashSet;
use tracing;

#[derive(Parser)]
struct Cli {
    #[clap(short, long, default_value = "config.json")]
    config: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    path_common_name_max_length: u32,
    hat_only_players: Vec<String>,
    number_of_multi_port_controllers_to_use: u32,
    controller_bindings_file: String,
    binding_names_file: String,
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

pub enum GameState {
    Binding,
    TeamSelect,
    GameActive,
}

use tracing::Level;
use tracing_subscriber::FmtSubscriber;

fn main() {
    // Initialize the subscriber to listen to all logs with DEBUG level and above
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG) // Explicitly set to DEBUG level
        .with_ansi(true) // Enable colored output
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let args = Cli::parse();
    let config = serde_json::from_str::<Config>(&std::fs::read_to_string(&args.config).unwrap())
        .expect("Failed to parse config file");
    dbg!(&config);

    // Read configuration file .json file
    let mut mpl = joypaths::MinimalPathLookup::read_from_disk(&config.controller_bindings_file);
    mpl.add_missing_paths_for_joys(&config);

    let mut minimal_paths: Vec<&String> = mpl.0.keys().collect();
    minimal_paths.sort();
    minimal_paths.reverse();

    for path in minimal_paths.iter() {
        let joy = &mpl.0[*path];
        let name = if let Some(n) = &joy.common_name {
            n
        } else {
            "None"
        };
        println!("{: <15} -> {: <20}", name, path);
    }

    let frozen_path = "teamlock.json";
    // Check for a teamlock.json file
    let mut frozen = if std::path::Path::new(&frozen_path).exists() {
        // If it exists, read it and return it
        let frozen =
            serde_json::from_str::<TeamLock>(&std::fs::read_to_string(&frozen_path).unwrap())
                .expect("Failed to parse frozen file");
        frozen
    } else {
        let team = Team {
            name: "Etherial Narwhals".to_owned(),
            players: vec![],
            out_index: 0,
        };
        let tl = TeamLock { teams: vec![team] };
        tl
    };
    dbg!(&frozen);

    // Check frozen
    let mut total_player_count = 0;
    let mut missing_players = Vec::new();
    for team in frozen.teams.iter() {
        for player in team.players.iter() {
            let mut fail = true;
            for joy in mpl.0.values() {
                if let Some(cn) = &joy.common_name {
                    if cn == player {
                        fail = false;
                        break;
                    }
                }
            }
            if fail {
                missing_players.push(player.clone());
            }
            total_player_count += 1;
        }
    }

    if missing_players.len() > 0 {
        println!("Missing players:");
        tracing::warn!("Missing players. Removing: {:?}", missing_players);

        let missing_set: HashSet<String> = missing_players.into_iter().collect();
        for team in frozen.teams.iter_mut() {
            team.players.retain(|player| !missing_set.contains(player));
        }
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
    let mut event_path_lookup = joypaths::EventPathLookup::repath(&config);

    let mut gui_teams = Vec::new();
    for team in frozen.teams.iter() {
        gui_teams.push(team.name.clone());
    }
    use mjoy_gui::gui::Ui;
    // let mut ui = Ui::new(
    //     gui_teams.as_slice(),
    //     mjoy_gui::gui::WidthHeight::new(1920, 1080),
    // );

    struct TopContext<'a> {
        fbinfo: FeedbackInfo,
        all_joys: outjoy::Outjoys<'a>,
    }

    let mut top_context = RefCell::new(Some(TopContext {
        fbinfo: update_gui_teams(&frozen),
        all_joys: outjoy::Outjoys::new(&frozen),
    }));

    let mut thresh = 0.9f32;
    let mut change_thresh_time = std::time::Instant::now() + std::time::Duration::from_secs(1);
    let mut gui_render_time = std::time::Instant::now();
    let mut game_state: GameState = GameState::Binding;
    let mut binder = crate::bindings::Binder::new(config.binding_names_file.clone());
    loop {
        let event = gilrs.next_event();

        match &event {
            Some(gilrs::Event {
                event: gilrs::EventType::Connected | gilrs::EventType::Disconnected,
                ..
            }) => {
                event_path_lookup = joypaths::EventPathLookup::repath(&config);
                mpl.add_missing_paths_for_joys(&config);
                continue;
            }
            _ => {}
        }

        // Have gilrs process all events so the cached state is as up to date as possible
        if event.is_some() {
            continue;
        }

        match game_state {
            GameState::GameActive => {
                let TopContext {
                    mut fbinfo,
                    all_joys,
                } = top_context.replace(None).unwrap();

                all_joys.update(&mut outjoy::UpdateContext {
                    minimal_path_lookup: &mpl,
                    gilrs: &mut gilrs,
                    event_path_lookup: &event_path_lookup,
                    feedback: &mut fbinfo,
                    hat_only_player_names: &config.hat_only_players,
                    button_threshold: thresh,
                });

                let now = std::time::Instant::now();
                if now.checked_duration_since(change_thresh_time).is_some() {
                    change_thresh_time = change_thresh_time + {
                        // Random number up to 5000
                        let random_millis = rand::random::<u64>() % 5000;
                        let random_millis = random_millis + 300;
                        std::time::Duration::from_millis(random_millis)
                    };
                    if now < change_thresh_time {
                        change_thresh_time = now + std::time::Duration::from_secs(1);
                    }
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

                top_context.replace(Some(TopContext { fbinfo, all_joys }));
            }
            GameState::Binding => {
                binder.update(&gilrs, &event_path_lookup, &mut mpl);
            }
            GameState::TeamSelect => {
                top_context.replace(None);

                let changed = team_select::mutate_team_selection(
                    &mut frozen,
                    &event_path_lookup,
                    &mpl,
                    &mut gilrs,
                );
                if changed {
                    let fbinfo = update_gui_teams(&frozen);
                    let all_joys = outjoy::Outjoys::new(&frozen);
                    let new_context = TopContext { fbinfo, all_joys };
                    top_context.replace(Some(new_context));
                }
            }
        };

        // if std::time::Instant::now()
        //     .checked_duration_since(gui_render_time)
        //     .is_some()
        // {
        //     gui_render_time = std::time::Instant::now() + std::time::Duration::from_millis(50);
        //     ui.render(&fbinfo, false);
        // } else {
        //     continue;
        // }
    }
}

fn update_gui_teams(frozen: &TeamLock) -> mjoy_gui::gui::feedback_info::FeedbackInfo {
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
            team_name: team.name.clone(),
            players: fbplayers,
            feedback: feedback.clone(),
        };

        fbteams.push(fb_team);
    }
    let mut fbinfo = mjoy_gui::gui::feedback_info::FeedbackInfo { teams: fbteams };

    fbinfo
}
