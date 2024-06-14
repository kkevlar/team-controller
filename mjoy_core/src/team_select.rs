pub fn mutate_team_selection(
    teams: &mut crate::TeamLock,
    epl: &crate::joypaths::EventPathLookup,
    mpl: &crate::joypaths::MinimalPathLookup,
    gilrs: &mut gilrs::Gilrs,
) -> bool {
    let mut changed = false;
    for (_id, gamepad) in gilrs.gamepads() {
        let devpath = gamepad.devpath();
        let minimal_path = epl.0.get(devpath);
        if minimal_path.is_none() {
            continue;
        }
        let minimal_path = minimal_path.unwrap();
        let named_path = mpl.0.get(minimal_path);
        if named_path.is_none() {
            continue;
        }
        let named_path = named_path.unwrap();
        let Some(common_name) = named_path.common_name.as_ref() else { continue; };

        let mut current_team_index = None;
        for (i, team) in teams.teams.iter_mut().enumerate() {
            if team.players.contains(common_name) {
                current_team_index = Some(i);
            }
        }

        // Handle button presses for A and B
        let button_a = crate::injoy::NamedButton::A;
        let button_b = crate::injoy::NamedButton::B;
        let button_id_a: gilrs::Button = crate::injoy::snes_namedbutton_to_id(&button_a);
        let button_id_b: gilrs::Button = crate::injoy::snes_namedbutton_to_id(&button_b);

        let value_a = gamepad
            .button_data(button_id_a)
            .map_or(0.0, |data| data.value());
        let value_b = gamepad
            .button_data(button_id_b)
            .map_or(0.0, |data| data.value());

        if value_b > 0.9 {
            // Remove player from their current team
            if let Some(index) = current_team_index {
                teams.teams[index]
                    .players
                    .retain(|player| player != common_name);
            }
            changed = true;
            continue;
        }

        if value_a > 0.9 {
            if current_team_index.is_none() {
                // Assign to team 0 if they don't have a team
                teams.teams[0].players.push(common_name.clone());
                changed = true;
                continue;
            }
        }

        let axes = [gilrs::Button::DPadRight, gilrs::Button::DPadUp];
        let mut values = [0, 0];
        for (i, axis) in axes.iter().enumerate() {
            let value = gamepad.button_data(*axis);
            let value = match value {
                Some(value) => {
                    let vv = value.value();
                    let vvv = match vv {
                        v if v < 0.1 => -1,
                        v if v > 0.9 => 1,
                        _ => 0,
                    };
                    vvv
                }
                None => 0,
            };
            values[i] = value;
        }

        let desired_team_index = match (current_team_index, (values[0], values[1])) {
            (None, _) => None,
            (Some(0), (1, _)) => Some(1),
            (Some(0), (_, 1)) => Some(2),
            (Some(1), (-1, _)) => Some(0),
            (Some(1), (_, 1)) => Some(3),
            (Some(2), (1, _)) => Some(3),
            (Some(2), (_, -1)) => Some(0),
            (Some(3), (-1, _)) => Some(2),
            (Some(3), (_, -1)) => Some(1),
            (Some(_), _) => None,
        };

        if let Some(new_index) = desired_team_index {
            if let Some(current_index) = current_team_index {
                if current_index != new_index {
                    changed = true;
                    teams.teams[current_index]
                        .players
                        .retain(|player| player != common_name);
                    teams.teams[new_index].players.push(common_name.clone());
                }
            }
        }
    }
    changed
}
