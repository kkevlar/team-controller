use gilrs;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::time::{Duration, Instant};

#[derive(Copy, Clone)]
pub enum UpdateState {
    Done,
    Binding,
}

pub struct Binder {
    bindings_filepath: String,
    bindings_to_make: Option<Vec<String>>,
    next_binding_allowed_time: Option<Instant>,
    cached_state: UpdateState,
}

impl Binder {
    pub fn new(bindings_filepath: String) -> Self {
        Binder {
            bindings_filepath,
            bindings_to_make: None,
            next_binding_allowed_time: None,
            cached_state: UpdateState::Binding,
        }
    }

    pub fn update(
        &mut self,
        gilrs: &gilrs::Gilrs,
        event_path_lookup: &crate::joypaths::EventPathLookup,
        mpl: &mut crate::joypaths::MinimalPathLookup,
    ) -> UpdateState {
        // If bindings_to_make is None, read from the filepath
        if self.bindings_to_make.is_none() {
            let file = File::open(&self.bindings_filepath).expect("Unable to open file");
            let reader = BufReader::new(file);
            let mut bindings = Vec::new();

            for line in reader.lines() {
                let line = line.expect("Unable to read line");
                bindings.push(line);
            }

            self.bindings_to_make = Some(bindings);
        }

        // Check if binding is allowed now
        if self.next_binding_allowed_time.is_none()
            || self.next_binding_allowed_time.unwrap() <= Instant::now()
        {
            if let Some(candidate_binding) = self.bindings_to_make.as_mut().unwrap().last().cloned()
            {
                tracing::info!("I'm trying to bind {}", candidate_binding);
                match self.perform_candidate_binding(
                    &candidate_binding,
                    gilrs,
                    event_path_lookup,
                    mpl,
                ) {
                    Ok(_) => {
                        // Binding was successful, remove the candidate from the list and update the time
                        self.bindings_to_make.as_mut().unwrap().pop();
                        self.next_binding_allowed_time =
                            Some(Instant::now() + Duration::from_millis(250));
                    }
                    Err(_) => {
                        // Binding was not successful, do not pop the candidate and do not update the time
                    }
                }
            } else {
                // No more bindings to process
                self.cached_state = UpdateState::Done;
            }
        }

        self.cached_state
    }

    pub fn perform_candidate_binding(
        &mut self,
        candidate_binding: &str,
        gilrs: &gilrs::Gilrs,
        event_path_lookup: &crate::joypaths::EventPathLookup,
        mpl: &mut crate::joypaths::MinimalPathLookup,
    ) -> Result<(), ()> {
        for (_id, gamepad) in gilrs.gamepads() {
            let button_a = crate::injoy::NamedButton::A;
            let button_b = crate::injoy::NamedButton::B;
            let button_id_a: gilrs::Button = crate::injoy::snes_namedbutton_to_id(&button_a);
            let button_id_b: gilrs::Button = crate::injoy::snes_namedbutton_to_id(&button_b);

            // Correctly obtaining button data
            let value_a = gamepad
                .button_data(button_id_a)
                .map_or(0.0, |data| data.value());
            let value_b = gamepad
                .button_data(button_id_b)
                .map_or(0.0, |data| data.value());

            // Check if button B is pressed to skip
            if value_b > 0.9 {
                return Ok(());
            }

            // Check if button A is pressed to perform the binding
            if value_a > 0.9 {
                let devpath = gamepad.devpath();

                // Correct lookup and mutation process
                if let Some(devpath_key) = event_path_lookup.0.get(devpath) {
                    if let Some(named_path) = mpl.0.get_mut(devpath_key) {
                        named_path.common_name = Some(candidate_binding.to_string());
                        return Ok(());
                    }
                }
            }
        }
        Err(())
    }
}
