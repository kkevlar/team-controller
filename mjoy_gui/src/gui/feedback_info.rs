#[derive(Clone)]
pub struct FeedbackInfo {
    pub teams: Vec<Team>,
}
#[derive(Clone)]
pub struct Player {
    pub player_name: String,
    pub feedback: Presses,
}
#[derive(Clone, PartialEq, Eq)]
pub enum PressState {
    Pressed,
    Unpressed,
}
#[derive(Clone)]
pub struct ButtonPress {
    pub button: String,
    pub state: PressState,
}
#[derive(Clone)]
pub struct Presses(pub Vec<ButtonPress>);

#[derive(Clone)]
pub struct Team {
    pub team_name: String,
    pub players: Vec<Player>,
    pub feedback: Presses,
}
