use kiss3d::light::Light;
use kiss3d::window::Window;

use crate::manipulate_emulator;

use self::feedback_info::FeedbackInfo;
pub mod feedback_info;
mod team_color;

struct Hz(f32);
impl Hz {
    fn new(hz: f32) -> Hz {
        Hz(hz)
    }
    fn apply(&self, duration: &std::time::Duration) -> f32 {
        (duration.as_secs_f32() * self.0 * 3.14159265).cos()
    }
}

pub struct Ui {
    window: Window,
    width_height: WidthHeight,
    teams: Vec<String>,
    logos: Vec<kiss3d::scene::PlanarSceneNode>,
    logos_locations: Vec<kiss3d::nalgebra::Translation2<f32>>,
    font: std::rc::Rc<kiss3d::text::Font>,
    colors: team_color::ColoredTeams,
    did_gui_off: bool,
    did_gui_on: bool,
}

pub struct WidthHeight {
    pub width: u32,
    pub height: u32,
}
impl WidthHeight {
    pub fn new(width: u32, height: u32) -> WidthHeight {
        WidthHeight { width, height }
    }
}

const XRATIO_DENOM: f32 = 1920f32;
const YRATIO_DENOM: f32 = 1080f32;

pub struct RatioXY<'a> {
    x: f32,
    y: f32,
    width_height: &'a WidthHeight,
}
impl<'a> RatioXY<'a> {
    pub fn new(x: f32, y: f32, width_height: &'a WidthHeight) -> RatioXY {
        RatioXY { x, y, width_height }
    }
    pub fn x(&'a self) -> f32 {
        self.x * (self.width_height.width as f32) / XRATIO_DENOM
    }
    pub fn y(&'a self) -> f32 {
        self.y * (self.width_height.height as f32) / YRATIO_DENOM
    }
}

enum SubtextInfo {
    Myself,
    Button(i32),
}

struct DrawPlayerInfo {
    player_index: usize,
}

enum TeamOrPlayer {
    Team,
    Player(DrawPlayerInfo),
}

struct DrawTextInfo<'a> {
    team_index: usize,
    team_or_player: TeamOrPlayer,
    color_index: usize,
    text: &'a str,
    sub: SubtextInfo,
    color_override: bool,
}

const TEXTURE_SIZE: f32 = 220f32;

impl Ui {
    pub fn new(teams: &[String], width_height: WidthHeight) -> Ui {
        let mut window =
            Window::new_with_size("Cool project", width_height.width, width_height.height);
        window.set_background_color(0.1, 0.1, 0.1);
        window.set_light(Light::StickToCamera);

        let texture_size = RatioXY::new(TEXTURE_SIZE, TEXTURE_SIZE, &width_height);
        let texture_position = RatioXY::new(845f32, 260f32, &width_height);
        let texture_position_bonus = RatioXY::new(0f32, 150f32, &width_height);

        let mut logos: Vec<_> = Vec::new();
        let mut trans: Vec<_> = Vec::new();
        for (i, team) in teams.iter().enumerate() {
            let mut r = window.add_rectangle(texture_size.x(), texture_size.y());
            let translate = &kiss3d::nalgebra::Translation2::new(
                texture_position.x() * if i % 2 == 0 { -1 } else { 1 } as f32
                    + texture_position_bonus.x(),
                texture_position.y() * if i < 2 { 1 } else { -1 } as f32
                    + texture_position_bonus.y(),
            );
            r.append_translation(translate);
            trans.push(translate.to_owned());
            let path = format!("./resources/images/{}.jpg", team);
            r.set_texture_from_file(std::path::Path::new(&path), team);
            logos.push(r);
        }

        let colors = {
            let hc = team_color::HintedColors::new();
            let teams: Vec<team_color::Team> = teams.iter().map(|t| team_color::Team(t)).collect();
            hc.color_teams(&teams.as_slice())
        };

        let ui = Ui {
            window,
            teams: teams.iter().map(|t| t.to_string()).collect(),
            logos,
            logos_locations: trans,
            colors,
            font: kiss3d::text::Font::new(std::path::Path::new("./resources/impact.ttf")).unwrap(),
            width_height,
            did_gui_on: false,
            did_gui_off: false,
        };
        ui
    }

    pub fn render(
        &mut self,
        feedback: &FeedbackInfo,
        show_logos: bool,
        bonus_name: Option<String>,
    ) {
        if let Some(name) = bonus_name {
            self.window.draw_text(
                &name,
                &kiss3d::nalgebra::Point2::new(20.0, 20.0),
                60.0,
                &self.font,
                &kiss3d::nalgebra::Point3::new(1.0, 1.0, 1.0),
            );
        }

        if show_logos && !self.did_gui_on {
            manipulate_emulator::resize::resize_and_focus_matching(
                &regex::Regex::new("Dolphin.*FPS").unwrap(),
            );
            manipulate_emulator::mute::unmute("dolphin-emu");
            self.did_gui_on = true;
        }
        if !show_logos && !self.did_gui_off {
            manipulate_emulator::mute::mute("dolphin-emu");
            self.did_gui_off = true;
        }

        self.logos
            .iter_mut()
            .for_each(|l| l.set_visible(show_logos));

        for (i, team) in feedback.teams.iter().enumerate() {
            let color_idx = self
                .colors
                .0
                .iter()
                .position(|c| c.team == team.team_name)
                .unwrap();
            assert!(self.teams[i] == team.team_name);

            let mut draw_text_info = DrawTextInfo {
                team_index: i,
                team_or_player: TeamOrPlayer::Team,
                color_index: color_idx,
                text: &team.team_name,
                sub: SubtextInfo::Myself,
                color_override: !show_logos,
            };
            if show_logos {
                self.draw_text(&draw_text_info);
                for (i, fb) in team.feedback.0.iter().enumerate() {
                    if fb.state == feedback_info::PressState::Unpressed {
                        continue;
                    }

                    draw_text_info.text = &fb.button;
                    draw_text_info.sub = SubtextInfo::Button(i as i32);
                    self.draw_text(&draw_text_info);
                }
            }

            for (i, player) in team.players.iter().enumerate() {
                draw_text_info.team_or_player =
                    TeamOrPlayer::Player(DrawPlayerInfo { player_index: i });
                draw_text_info.text = &player.player_name;
                draw_text_info.sub = SubtextInfo::Myself;
                self.draw_text(&draw_text_info);
                for (i, fb) in player.feedback.0.iter().enumerate() {
                    if fb.state == feedback_info::PressState::Unpressed {
                        continue;
                    }

                    draw_text_info.text = &fb.button;
                    draw_text_info.sub = SubtextInfo::Button(i as i32);
                    self.draw_text(&draw_text_info);
                }
            }
        }
        self.window.render();
    }

    fn draw_text(&mut self, info: &DrawTextInfo) {
        use TeamOrPlayer::*;

        let xpos = self.width_height.width as f32
            + (-250f32 * self.width_height.width as f32 / XRATIO_DENOM)
            + match info.team_or_player {
                Team => 0f32,
                Player(_) => (50f32 * self.width_height.width as f32) / XRATIO_DENOM,
            }
            + (self.logos_locations[info.team_index].x)
                * (1.95f32
                    + match info.team_or_player {
                        Team => 0f32,
                        Player(_) => 0f32,
                    })
            + match info.sub {
                SubtextInfo::Myself => 0f32,
                SubtextInfo::Button(i) => i as f32 + 0.7f32,
            } * (35f32 * self.width_height.width as f32)
                / XRATIO_DENOM;

        let ypos = self.width_height.height as f32
            + (158f32 * self.width_height.height as f32) / YRATIO_DENOM
            + (self.logos_locations[info.team_index].y) * -2f32
            + (63f32
                * match info.team_or_player {
                    Team => 0 as f32,
                    Player(DrawPlayerInfo { player_index, .. }) => {
                        (player_index as f32 * 1.8 as f32) + 1.9 as f32
                    }
                }
                * self.width_height.height as f32)
                / YRATIO_DENOM
            + match info.sub {
                SubtextInfo::Myself => 0,
                SubtextInfo::Button(_) => 1,
            } as f32
                * (60f32 * self.width_height.height as f32)
                / YRATIO_DENOM;

        let size = (match info.team_or_player {
            Team => 85,
            Player(_) => 70,
        } as f32
            * match info.sub {
                SubtextInfo::Myself => 1f32,
                SubtextInfo::Button(_) => 0.9f32,
            }
            * self.width_height.width as f32)
            / XRATIO_DENOM;

        let color = self.colors.0[info.color_index].color.0;
        let color = color
            * match info.sub {
                SubtextInfo::Myself => 1f32,
                SubtextInfo::Button(_) => 0.9f32,
            };
        let color = color
            * match info.team_or_player {
                Team => 1f32,
                Player(_) => 0.9f32,
            };
        let mut color = color;

        if info.color_override {
            color = kiss3d::nalgebra::Vector3::new(1f32, 1f32, 1f32).into();
        }

        self.window.draw_text(
            info.text,
            &kiss3d::nalgebra::Point2::new(xpos, ypos),
            size,
            &self.font,
            &color,
        );
    }
}

//pub fn do_cubes() {
////env_logger::init();

//let mut cubes = Vec::new();
//for i in 0..5 {
//cubes.push(window.add_cube(1.0, 1.0, 1.0));
//}

//let len = cubes.len();
//for (i, cube) in cubes.iter_mut().enumerate() {
//let diff = (i as f32 - (len as f32 / 2f32)) as f32;
//let diff = diff * 1f32;
//let tx = kiss3d::nalgebra::Translation3::new(0f32, diff, 10.0);
//cube.append_translation(&tx);
//cube.set_color(1.0, diff, 0.0);
//}

//let rot = UnitQuaternion::from_axis_angle(&Vector3::y_axis(), 0.014);

////crate::manipulate_emulator::mute::mute("dolphin-emu");
////crate::manipulate_emulator::resize::resize_and_focus_matching(
////&regex::Regex::new(r"[Dd]olphin.*FPS").unwrap(),
////);

//// Record start time
//let start_time = std::time::SystemTime::now();

//let square_lateral_hz = Hz::new(1f32);

//let font = kiss3d::text::Font::new(std::path::Path::new("./impact.ttf")).unwrap();

//let wh = wordhash::Wordhash::new(134, 134);
//let team_names: Vec<String> = (0i32..10i32)
//.map(|i| wh.team_name(&i.to_be_bytes()))
//.collect();
//let teams: Vec<team_color::Team> = team_names.iter().map(|n| team_color::Team(n)).collect();
//let hc = team_color::HintedColors::new();
//let colors_to_use = hc.color_teams(&teams.as_slice());

//while window.render() {
//for (num, c) in colors_to_use.iter().enumerate() {
//window.draw_text(
//c.team.0,
//&kiss3d::nalgebra::Point2::new(10f32, 10f32 + 200f32 * num as f32),
//110.0,
//&font,
//&c.color.0,
//);
//window.draw_text(
//"<",
//&kiss3d::nalgebra::Point2::new(10f32, 90f32 + 200f32 * num as f32),
//90.0,
//&font,
//&c.color.0,
//);
//window.draw_text(
//if num % 2 == 0 { "ABXYLR" } else { "A XY R" },
//&kiss3d::nalgebra::Point2::new(80f32, 90f32 + 200f32 * num as f32),
//90.0,
//&font,
//&c.color.0,
//);
//}
//for num in 0..10 {
//window.draw_text(
//"Hello birds!",
//&kiss3d::nalgebra::Point2::new(3400f32, 10f32 + 200f32 * num as f32),
//110.0,
//&font,
//&kiss3d::nalgebra::Point3::new(0.6, 0f32, 0.9),
//);
//}
//let duration = start_time.elapsed().unwrap();

//for (i, cube) in cubes.iter_mut().enumerate() {
//let diff = (i as f32 - (len as f32 / 2f32)) as f32;
//let diff = diff * 1f32;

//cube.set_local_translation(kiss3d::nalgebra::Translation3::new(
//square_lateral_hz.apply(&duration) * 10f32,
//diff,
//10.0,
//));
//cube.prepend_to_local_rotation(&rot);
//}
//}
//}
