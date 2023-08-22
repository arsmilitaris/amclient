// (C) Copyright 2023 Ars Militaris Dev

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy::asset::AssetPath;
use bevy::winit::WinitWindows;
use bevy::reflect::std_traits::ReflectDefault;
use bevy::ecs::schedule::SystemConfig;
use winit::window::Icon;

use std::fs;
use std::path::{Path};
use std::fs::File;
use std::io::Write;
use std::collections::HashMap;

use csv::Reader;
use csv::StringRecord;

use bevy_quinnet::{
    client::{
        certificate::CertificateVerificationMode, Client, connection::ConnectionConfiguration, connection::ConnectionEvent,
        QuinnetClientPlugin, 
    },
    shared::ClientId,
};

use bevy_console::{ConsoleConfiguration, ConsolePlugin, ToggleConsoleKey, PrintConsoleLine, ConsoleOpen};
use bevy_console::{reply, AddConsoleCommand, ConsoleCommand,};
use clap::{Parser};

use bevy_inspector_egui::prelude::*;
use bevy_inspector_egui::quick::ResourceInspectorPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;

use bevy::log::LogPlugin;
use bevy_log::FileAppenderSettings;
use bevy_log::Rolling;

use serde::{Deserialize, Serialize};

use rand::Rng;

use pathfinding::prelude::astar;
use std::cell::RefCell;

use slog::{o, Drain, Logger, Record, Level, OwnedKVList, KV};
use slog_term::{FullFormat, TermDecorator, PlainDecorator};
use slog_async::Async;
use std::sync::Mutex;
use std::error::Error;
use std::thread_local;

#[derive(Reflect)]
enum UnitAction {
	Move {
		origin: Pos,
		destination: Pos,
		timer: Timer,
	},
	Talk {
		message: String,
	},
	BasicAttack {
		target: Pos,
		is_counterattack: bool,
		damage: usize,
	},
	DoNothing,
}

impl Default for UnitAction {
    fn default() -> Self {
        UnitAction::DoNothing
    }
}

#[derive(Serialize, Deserialize)]
enum ClientMessage {
	GetClientId,
	StartGame,
	LoadingComplete,
	WaitTurnComplete,
	Wait,
	Move {
		origin: Pos,
		destination: Pos,
	},
	BasicAttack {
		attacker: Pos,
		target: Pos,
		damage: usize,
	},
}

#[derive(Serialize, Deserialize)]
enum ServerMessage {
	ClientId {
		client_id: ClientId,
	},
	StartGame {
		client_id: ClientId,
	},
	StartGame2,
	PlayerTurn {
		client_id: ClientId,
		current_unit: usize,
	},
	WaitTurn {
		wait_turns: Vec<(UnitId, WTCurrent)>,
	},
	Wait,
	Move {
		origin: Pos,
		destination: Pos,
	},
	BasicAttack {
		attacker: Pos,
		target: Pos,
		damage: usize,
		is_counterattack: bool,
	},
	GameOver {
		winner: ControlledBy,
	}
}

#[derive(Reflect)]
#[reflect(Default)]
enum Direction {
	East,
	South,
	West,
	North,
}

impl Direction {
	fn from_string(dir_string: String) -> Direction {
		
		match dir_string.as_str() {
			"East" => Direction::East,
			"South" => Direction::South,
			"West" => Direction::West,
			"North" => Direction::North,
			_ => panic!("Invalid Direction string: {}", dir_string),
		}
	}
}

impl Default for Direction {
	fn default() -> Self {
        Direction::East
    }
}

// CONSOLE

/// DoNothing command
#[derive(Parser, ConsoleCommand)]
#[command(name = "nothing")]
struct DoNothingCommand {
    
}

/// Talk command
#[derive(Parser, ConsoleCommand)]
#[command(name = "talk")]
struct TalkCommand {
	/// The `UnitId` of the unit that will say the message.
	unit_id: usize,
	
	/// The message to say.
	msg: String,
}

/// Move command
#[derive(Parser, ConsoleCommand)]
#[command(name = "move")]
struct MoveCommand {
	/// The `UnitId` of the unit that will say the message.
	unit_id: usize,
	
	/// The X map coordinate to move the unit into.
	x: usize,
	
	/// The Y map coordinate to move the unit into.
	y: usize,
}

// COMPONENTS

#[derive(Component)]
struct MainMenuUI {}

#[derive(Component)]
struct StartDemoButton {}

#[derive(Component)]
struct StartAmbushButton {}

#[derive(Component)]
struct StartMultiplayerButton {}

#[derive(Component)]
struct QuitGameButton {}

#[derive(Component)]
struct NakedSwordsman {

}

#[derive(Component)]
struct CurrentUnit {

}

#[derive(Component)]
struct Attacker {}

#[derive(Component)]
struct Target {}

#[derive(Component)]
struct MoveTile {}

#[derive(Component)]
struct MoveTiles {
	move_tiles: Vec<Pos>,
}

#[derive(Component)]
struct AttackTile {}

#[derive(Component)]
struct AttackTiles {
	attack_tiles: Vec<Pos>,
}

#[derive(Component)]
struct MoveActions {
	move_actions: Vec<MoveAction>,
}

#[derive(Component)]
struct MoveAction {
	origin: Pos,
	destination: Pos,
	timer: Timer,
}

#[derive(Component)]
struct TalkAction {
	message: String,
}

#[derive(Component)]
struct BasicAttackAction {
	target: Pos,
	is_counterattack: bool,
	damage: usize,
}

#[derive(Component)]
struct DoNothingAction;

#[derive(Component, Reflect, Default)]
struct UnitActions {
	unit_actions: Vec<UnitActionTuple>,
	processing_unit_action: bool,
}

#[derive(Reflect)]
#[reflect(Default)]
struct UnitActionTuple(UnitAction, f32);

impl Default for UnitActionTuple {
    fn default() -> Self {
        UnitActionTuple(UnitAction::DoNothing, 0.0)
    }
}

#[derive(Component)]
struct TalkUI {

}

#[derive(Component)]
struct Cursor {
	x: usize,
	y: usize,
}

#[derive(Component, Clone)]
struct Map {
	map: Vec<Vec<(usize, TileType, Vec<Entity>, Vec<Entity>)>>,
}

#[derive(Component)]
struct Tile;

#[derive(Component, Debug, Clone, PartialEq)]
enum TileType {
	Grass, 
}

#[derive(Component)]
struct GameText;

#[derive(Component)]
struct Unit;

#[derive(Component, Clone, Reflect, Default, Eq, PartialEq, Hash, Copy, Debug, Serialize, Deserialize)]
#[reflect(Default)]
struct Pos {
	x: usize,
	y: usize,
}

#[derive(Component, Clone, Serialize, Deserialize, Debug)]
struct UnitId { value: usize, }

impl Default for UnitId {
	fn default() -> Self {
        UnitId {
            value: 1,
        }
    }
}
#[derive(Component)]
struct UnitTeam { value: usize, }

#[derive(Component)]
struct UnitName { value: String, }

#[derive(Component)]
struct UnitClass { value: String, }

#[derive(Component)]
struct PosX { value: usize, }

#[derive(Component)]
struct PosY { value: usize, }

#[derive(Component)]
struct WTMax { value: usize, }

#[derive(Component, Reflect, Clone, Serialize, Deserialize, Debug)]
struct WTCurrent { value: usize, }

#[derive(Component)]
struct HPMax { value: usize, }

#[derive(Component, Reflect, InspectorOptions)]
struct HPCurrent { value: usize, }

#[derive(Component)]
struct MPMax { value: usize, }

#[derive(Component)]
struct MPCurrent { value: usize, }

#[derive(Component)]
struct STR { value: usize, }

#[derive(Component)]
struct VIT { value: usize, }

#[derive(Component)]
struct INT { value: usize, }

#[derive(Component)]
struct MEN { value: usize, }

#[derive(Component)]
struct AGI { value: usize, }

#[derive(Component)]
struct DEX { value: usize, }

#[derive(Component)]
struct LUK { value: usize, }

#[derive(Component)]
struct UnitSprite { value: String, }

#[derive(Component, Default, Reflect)]
#[reflect(Default)]
struct DIR { direction: Direction, }

#[derive(Component, Default, Reflect)]
#[reflect(Default)]
struct MovementRange { value: isize, }

#[derive(Component, Default, Reflect)]
#[reflect(Default)]
struct AttackRange { value: isize, }

#[derive(Component, Default, Reflect, Clone, Copy)]
#[reflect(Default)]
enum AttackType {
	#[default]
	Melee,
	Ranged,
}

impl AttackType {
	fn from_string(string: String) -> AttackType {
		match string.as_str() {
			"Melee" => { return AttackType::Melee; },
			"Ranged" => { return AttackType::Ranged; },
			_ => { panic!("Invalid AttackType string: {}.", string); },
		}
	}
}

#[derive(Bundle)]
struct UnitAttributes {
	unit_id: UnitId,
	unit_team: UnitTeam,
	unit_name: UnitName,
	unit_class: UnitClass,
	pos_x: PosX,
	pos_y: PosY,
	wt_max: WTMax,
	wt_current: WTCurrent,
	hp_max: HPMax,
	hp_current: HPCurrent,
	mp_max: MPMax,
	mp_current: MPCurrent,
	str: STR,
	vit: VIT,
	int: INT,
	men: MEN,
	agi: AGI,
	dex: DEX,
	luk: LUK,
	unit_sprite: UnitSprite,
	dir: DIR,
	movement_range: MovementRange,
	attack_range: AttackRange,
	attack_type: AttackType,
}

// STATES

#[derive(Reflect, States, Debug, Clone, Eq, PartialEq, Hash, Default)]
#[reflect(Default)]
enum GameState {
	#[default]
	MainMenu,
	Loading,
	LoadingComplete,
	Battle,
	Talk,
	Move,
	WaitTurn,
	Wait,
	LoadAmbush,
	Ambush,
	SinglePlayerPause,
	GameOver,
}

#[derive(Reflect, States, Debug, Clone, Eq, PartialEq, Hash, Default)]
#[reflect(Default)]
enum TurnState {
	#[default]
	Wait,
	Turn,
	ChooseMove,
	ChooseAttack,
	AI,
}

// EVENTS

#[derive(Event)]
struct GameStartEvent;

#[derive(Event)]
struct MapReadEvent {
	pub map: Vec<Vec<String>>,
}

#[derive(Event)]
struct MapSetupEvent;

#[derive(Event)]
struct UnitsReadEvent {
	pub units: Vec<StringRecord>,
}

#[derive(Event)]
struct UnitsGeneratedEvent;

// RESOURCES

#[derive(Resource)]
struct Game {
	current_unit: usize,
	current_team: usize,
	players: HashMap<usize, ControlledBy>,
	winner: ControlledBy,
	is_multiplayer: bool,
}

impl Default for Game {
	fn default() -> Self {
        Game {
            current_unit: 0,
            current_team: 1,
            players: HashMap::new(),
            winner: ControlledBy::None,
            is_multiplayer: false, 
        }
    }
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
enum ControlledBy {
	Player,
	AI,
	None,
}

#[derive(Resource, Default)]
struct ClientData {
	client_id: ClientId,
}

#[derive(Resource, Default)]
struct DemoData {
	current_unit: UnitId,
}

#[derive(Resource)]
struct Slog {
	logger: slog::Logger,
}

// Client & Server
fn main() {
	std::panic::set_hook(Box::new(custom_panic_hook));
	
//	// Setup the logger
//    let log = setup_logging();
	
    let mut app = App::new();
//  app.insert_resource(Slog { logger: log, });
	app.add_plugins(
		DefaultPlugins.set(WindowPlugin {
			primary_window: Some(Window {
				title: "Ars Militaris".into(),
				..default()
			}),
			..default()
		})
//		.disable::<LogPlugin>()
		.set(LogPlugin {
			file_appender_settings: Some(FileAppenderSettings {
				prefix: String::from("amclient.log"),
				rolling: Rolling::Minutely,
				..default()
			}),
			..default()
		})
	);
	app.add_plugin(QuinnetClientPlugin::default());
	app.add_plugins(ConsolePlugin)
		.insert_resource(ConsoleConfiguration {
			// Override config here.
			keys: vec![ToggleConsoleKey::KeyCode(KeyCode::Backslash)],
			left_pos: 0.0,
			top_pos: 600.0,
			height: 150.0,
			width: 400.0,
			..Default::default()
		});
	app.add_plugin(WorldInspectorPlugin::new());
	app.register_type::<ConsoleConfiguration>();
	app.register_type::<HPCurrent>();
	app.register_type::<UnitActions>();
	app.register_type::<UnitAction>();
	app.register_type::<UnitActionTuple>();
	app.register_type::<Pos>();
	app.register_type::<WTCurrent>();
	app.register_type::<DIR>();
	app.register_type::<MovementRange>();
	app.register_type::<AttackRange>();
	app.register_type::<AttackType>();
//	app.add_plugin(ResourceInspectorPlugin::<ConsoleConfiguration>::default());
//	app.add_plugin(ResourceInspectorPlugin::<State<GameState>>::default());
//	app.add_plugin(ResourceInspectorPlugin::<State<TurnState>>::default());
//	app.add_plugin(ResourceInspectorPlugin::<NextState<GameState>>::default());
//	app.add_plugin(ResourceInspectorPlugin::<NextState<TurnState>>::default());
	app.add_console_command::<DoNothingCommand, _>(do_nothing_command);
	app.add_console_command::<TalkCommand, _>(talk_command);
	app.add_console_command::<MoveCommand, _>(move_command);
	app.add_state::<GameState>();
	app.add_state::<TurnState>();
	app.add_event::<GameStartEvent>();
	app.add_event::<MapReadEvent>();
	app.add_event::<MapSetupEvent>();
	app.add_event::<UnitsReadEvent>();
	app.add_event::<UnitsGeneratedEvent>();
	app.init_resource::<Game>();
	app.init_resource::<ClientData>();
	app.init_resource::<DemoData>();
	
	if cfg!(windows) {
		app.add_systems(Startup, set_window_icon);
	} 
	
//	app.add_systems(Startup, test_slog);
	app.add_systems(OnEnter(GameState::MainMenu),
		(start_connection, send_get_client_id_message)
			.chain()
	);
	app.add_systems(OnEnter(GameState::MainMenu), setup_main_menu);
	app.add_systems(OnExit(GameState::MainMenu), tear_down_main_menu);
	app.add_systems(Update, handle_main_menu_buttons
		.run_if(in_state(GameState::MainMenu))
	);
	app.add_systems(Update,
		(send_start_game_message_system, handle_server_messages)
			.run_if(in_state(GameState::MainMenu))
	);
//	app.add_systems(Update,
//		//(read_map_system, setup_map_system, read_battle_system, generate_units_system,  place_units_on_map_system, handle_player_turn_server_message)
//		(read_map_system, setup_map_system, read_battle_system, (generate_units_system, apply_deferred).chain(),  place_units_on_map_system)
//			.run_if(in_state(GameState::Loading))
//	);
//	app.add_systems(OnExit(GameState::Loading), init_cursor_system);
//	app.add_systems(OnExit(GameState::Loading), setup_game_resource_system_multiplayer);
	app.add_systems(OnEnter(GameState::LoadingComplete), loading_complete);
//	app.add_systems(OnEnter(GameState::Battle), (apply_deferred, setup_cursor_system).chain());
	app.add_systems(Update,
		(end_turn_system, (apply_state_transition::<GameState>, handle_player_turn_server_message, apply_state_transition::<GameState>).chain())
			.run_if(in_state(GameState::Battle))
	);
	app.add_systems(Update, handle_player_turn_server_message
		.run_if(in_state(GameState::Wait))
	);
	app.add_systems(OnEnter(GameState::Loading), setup_grid_system);
	app.add_systems(OnEnter(GameState::Loading), setup_camera_system);
	app.add_systems(OnEnter(GameState::Loading), (apply_deferred, setup_text_system)
		.chain()
		.after(setup_grid_system)
	);
	app.add_systems(OnEnter(GameState::Loading), (apply_deferred, spawn_units)
		.chain()
		.after(setup_text_system)
	);
	app.add_systems(OnEnter(GameState::Loading), (apply_deferred, z_order_system)
		.chain()
		.after(setup_text_system)
	);
	app.add_systems(OnEnter(GameState::Loading), (apply_deferred, z_unit_order_system)
		.after(spawn_units)
	);
	app.add_systems(Update, set_loading_complete
		.run_if(in_state(GameState::Loading))
	);
	app.add_systems(OnExit(GameState::Loading), handle_unit_directions);
	app.add_systems(OnEnter(GameState::LoadAmbush), setup_grid_system);
	app.add_systems(OnEnter(GameState::LoadAmbush), setup_camera_system);
	app.add_systems(OnEnter(GameState::LoadAmbush), (apply_deferred, setup_text_system)
		.chain()
		.after(setup_grid_system)
	);
	app.add_systems(OnEnter(GameState::LoadAmbush), (apply_deferred, spawn_units)
		.chain()
		.after(setup_text_system)
	);
	app.add_systems(OnEnter(GameState::LoadAmbush), (apply_deferred, z_order_system)
		.chain()
		.after(setup_text_system)
	);
	app.add_systems(OnEnter(GameState::LoadAmbush), (apply_deferred, z_unit_order_system)
		.after(spawn_units)
	);
	app.add_systems(OnExit(GameState::LoadAmbush), handle_unit_directions);
	app.add_systems(OnExit(GameState::LoadAmbush), test_write_to_console);
	app.add_systems(OnExit(GameState::LoadAmbush), toggle_console);
	app.add_systems(Update, z_order_system
		.run_if(in_state(GameState::Ambush))
	);
	app.add_systems(Update, z_unit_order_system
		.run_if(in_state(GameState::Ambush))
	);
	app.add_systems(Update, handle_unit_directions
		.run_if(in_state(GameState::Ambush))
	);
	app.add_systems(Update, handle_unit_directions
		.run_if(in_state(GameState::Move))
	);
	app.add_systems(Update, handle_unit_death
		.run_if(in_state(GameState::Ambush))
	);
	app.add_systems(Update, handle_ambush_game_over
		.run_if(in_state(GameState::Ambush))
	);
	app.add_systems(Update, z_order_system
		.run_if(in_state(GameState::Battle))
	);
	app.add_systems(Update, z_unit_order_system
		.run_if(in_state(GameState::Battle))
	);
	app.add_systems(Update, handle_unit_directions
		.run_if(in_state(GameState::Battle))
	);
	app.add_systems(Update, handle_unit_death
		.run_if(in_state(GameState::Battle))
	);
	app.add_systems(Update, handle_ambush_game_over
		.run_if(in_state(GameState::Battle))
	);
	app.add_systems(OnTransition { from: GameState::Ambush, to: GameState::MainMenu, }, handle_ambush_to_main_menu_transition);
	app.add_systems(OnTransition { from: GameState::Battle, to: GameState::MainMenu, }, handle_ambush_to_main_menu_transition);
	app.add_systems(Update, position_cursor
		.run_if(in_state(TurnState::Turn))
	);
	app.add_systems(OnEnter(GameState::LoadAmbush), init_cursor_system);
	app.add_systems(OnEnter(GameState::Loading), init_cursor_system);
	app.add_systems(OnEnter(TurnState::Turn), setup_cursor_system_2);
	app.add_systems(OnEnter(TurnState::ChooseMove), setup_cursor_system_2);
	app.add_systems(OnExit(TurnState::Turn), hide_cursor);
	app.add_systems(Update, move_cursor_2
		.run_if(in_state(TurnState::Turn))
	);
	app.add_systems(Update, move_cursor_2
		.run_if(in_state(TurnState::ChooseMove))
	);
	app.add_systems(Update, position_cursor
		.run_if(in_state(TurnState::ChooseMove))
	);
	app.add_systems(Update, move_cursor_2
		.run_if(in_state(TurnState::ChooseAttack))
	);
	app.add_systems(Update, position_cursor
		.run_if(in_state(TurnState::ChooseAttack))
	);
	app.add_systems(Update, tick_move_timer
		.run_if(in_state(GameState::Move))
	);
	app.add_systems(Update, move_camera_system
		.run_if(in_state(GameState::Ambush))
	);
	app.add_systems(Update, move_camera_system
		.run_if(in_state(GameState::Move))
	);
	app.add_systems(Update, move_camera_system
		.run_if(in_state(GameState::Battle))
	);
	app.add_systems(Update, move_camera_system
		.run_if(in_state(GameState::Wait))
	);
	//app.add_systems(OnEnter(GameState::Ambush), ars_militaris_demo);
	app.add_systems(Update, single_player_pause);
	app.add_systems(Update, handle_single_player_pause_state
		.run_if(in_state(GameState::SinglePlayerPause))
	);
	app.add_systems(Update, wait_turn_system
		.run_if(in_state(GameState::Ambush))
		.run_if(not(in_state(TurnState::Turn)))
		.run_if(not(in_state(TurnState::ChooseMove)))
		.run_if(not(in_state(TurnState::ChooseAttack)))
		.run_if(not(in_state(TurnState::AI)))
	);
	app.add_systems(Update, end_turn_single_player
		.run_if(in_state(TurnState::Turn).and_then(is_singleplayer))
	);
	app.add_systems(Update, first_ai
		.run_if(in_state(TurnState::AI))
	);
	app.add_systems(OnEnter(TurnState::ChooseMove), choose_move);
	app.add_systems(Update, start_choose_move
		.run_if(in_state(TurnState::Turn))
	);
	app.add_systems(Update, handle_choose_move
		.run_if(in_state(TurnState::ChooseMove))
	);
	app.add_systems(OnEnter(TurnState::ChooseAttack), choose_attack);
	app.add_systems(Update, start_choose_attack
		.run_if(in_state(TurnState::Turn))
	);
	app.add_systems(Update, handle_choose_attack
		.run_if(in_state(TurnState::ChooseAttack))
	);
	app.add_systems(OnEnter(GameState::LoadAmbush), setup_game_resource_system);
//	app.add_systems(Update, move_gaul_warrior
//		.run_if(in_state(GameState::Ambush))
//		//.run_if(warrior_already_spawned)
//	);
	app.add_systems(Update, (process_unit_actions, apply_deferred)
		.chain()
		.run_if(in_state(GameState::Ambush))
	);
	app.add_systems(Update, (apply_deferred, process_move_actions, apply_deferred)
		.chain()
		.run_if(in_state(GameState::Ambush))
	);
	app.add_systems(Update, (apply_deferred, handle_move_state, apply_deferred)
		.chain()
		.run_if(in_state(GameState::Move))
	);
	app.add_systems(Update, (apply_deferred, process_talk_actions, apply_deferred)
		.chain()
		.run_if(in_state(GameState::Ambush))
	);
	app.add_systems(Update, handle_talk_state
		.run_if(in_state(GameState::Talk))
	);
	app.add_systems(Update, (apply_deferred, process_basic_attack_actions, apply_deferred)
		.chain()
		.run_if(in_state(GameState::Ambush))
	);
	app.add_systems(Update, (process_unit_actions, apply_deferred)
		.chain()
		.run_if(in_state(GameState::Battle))
	);
	app.add_systems(Update, (apply_deferred, process_move_actions, apply_deferred)
		.chain()
		.run_if(in_state(GameState::Battle))
	);
	app.add_systems(Update, (apply_deferred, process_talk_actions, apply_deferred)
		.chain()
		.run_if(in_state(GameState::Battle))
	);
	app.add_systems(Update, (apply_deferred, process_basic_attack_actions, apply_deferred)
		.chain()
		.run_if(in_state(GameState::Battle))
	);
	app.add_systems(Update, center_camera_on_unit
		.run_if(in_state(GameState::Move))
	);
	app.add_systems(Startup, get_toggle_console_key);
	app.run();
}

// SYSTEMS

// Client
fn set_window_icon(
    // we have to use `NonSend` here
    windows: NonSend<WinitWindows>,
    window_query: Query<(Entity, &Window), With<PrimaryWindow>>,
) {
	let (entity, window) = window_query.single();
	
	let primary = windows.get_window(entity).unwrap();
	
	// here we use the `image` crate to load our icon data from a png file
	// this is not a very bevy-native solution, but it will do
	let (icon_rgba, icon_width, icon_height) = {
		let image = image::open("amlogo.png")
			.expect("Failed to open icon path")
			.into_rgba8();
		let (width, height) = image.dimensions();
		let rgba = image.into_raw();
		(rgba, width, height)
	};

	let icon = Icon::from_rgba(icon_rgba, icon_width, icon_height).unwrap();

	primary.set_window_icon(Some(icon));
	
    

    
}

// Server
fn read_map_system(mut events: EventReader<GameStartEvent>, mut events2: EventWriter<MapReadEvent>) {
	
	for event in events.iter() {
		//info!("DEBUG: Reading map file...");
	
		let file_contents = fs::read_to_string("src/map.txt").unwrap();
		
		//info!("DEBUG: Read map file.");
		
		// Separate map into lines.
		let map_lines: Vec<&str> = file_contents.split('\n').collect();
		//info!("DEBUG: Map line 1 is: \n{}", map_lines[0]);
		
		// Separate lines and build 2D-array.
		info!("DEBUG: Starting to build 2D array of map...");
		let mut map: Vec<Vec<String>> = Vec::new();
		for i in 0..map_lines.len() {
			let mut map_line: Vec<String> = Vec::new();
			let line = map_lines[i];
			let line_splitted: Vec<&str> = line.split(' ').collect();
			for j in 0..line_splitted.len() {
				let map_cell = line_splitted[j].to_owned();
				map_line.push(map_cell);
			}
			map.push(map_line);
		}
		info!("DEBUG: Finished building 2D array of map.");
		
		//info!("DEBUG: Printing map file...");
		//info!("{}", file_contents);
		//info!("DEBUG: Printed map file...");
		
		events2.send(MapReadEvent {
						map: map,
					});
	}
}

// Client & Server
fn setup_map_system(mut events: EventReader<MapReadEvent>, mut events2: EventWriter<MapSetupEvent>, mut commands: Commands, asset_server: Res<AssetServer>) {
	
	for event in events.iter() {
		// Spawn camera.
		commands.spawn(Camera2dBundle::default());
		
		let map = &event.map;
		
		info!("DEBUG: Starting to set up map in the ECS World...");
		// For each cell in map, generate a 2D text and position it.
		for i in 0..map.len() {
			for j in 0..map[i].len() {
			
				// Compute position.
				let i_as_float = i as f32;
				let j_as_float = j as f32;
			
				// Spawn text.
				commands.spawn((
					TextBundle::from_section(
						map[i][j].as_str(),
						TextStyle {
							font: asset_server.load("fonts/FiraSans-Bold.ttf"),
							font_size: 80.0,
							color: Color::WHITE,
						},
					)
					.with_text_alignment(TextAlignment::Center)
					.with_style(Style {
						position_type: PositionType::Absolute,
						top: Val::Px(60.0 * i_as_float),
						right: Val::Px(60.0 * j_as_float),
						max_width: Val::Px(100.0),
						max_height: Val::Px(300.0),
						..default()
					}),
					Tile,
					Pos {
						x: i,
						y: j,
					},
				));
			}
		}
		info!("DEBUG: Finished setting up map in the ECS World.");
		events2.send(MapSetupEvent);
	}
}

// Server
fn read_battle_system(mut events: EventReader<MapSetupEvent>, mut events2: EventWriter<UnitsReadEvent>) {
	for event in events.iter() {
		let mut rdr = Reader::from_path("src/the_patrol_ambush_data.csv").unwrap();
		let mut records: Vec<StringRecord> = Vec::new();
		for result in rdr.records(){
			let record = result.unwrap();
			//info!("{:?}", record);
			records.push(record);
		}
		events2.send(UnitsReadEvent {
							units: records,
						});
	}
}

// Server
fn generate_units_system(mut events: EventReader<UnitsReadEvent>, mut events2: EventWriter<UnitsGeneratedEvent>, mut commands: Commands) {
	
	for event in events.iter() {
		// For each record, create an Entity for an unit.
		let records = &event.units;
		for record in records {
			info!("DEBUG: Creating new unit...");
			commands.spawn((
				UnitAttributes {
					unit_id : UnitId { value: record[0].parse().unwrap(), },
					unit_team : UnitTeam { value: record[1].parse().unwrap(), },
					unit_name : UnitName { value: record[2].to_string(), },
					unit_class : UnitClass { value: record[3].to_string(), },
					pos_x : PosX { value: record[4].parse().unwrap(), }, 
					pos_y : PosY { value: record[5].parse().unwrap(), },
					wt_max : WTMax { value: record[6].parse().unwrap(), },
					wt_current : WTCurrent{ value: record[7].parse().unwrap(), },
					hp_max : HPMax { value: record[8].parse().unwrap(), },
					hp_current : HPCurrent { value: record[9].parse().unwrap(), },
					mp_max : MPMax { value: record[10].parse().unwrap(), },
					mp_current : MPCurrent { value: record[11].parse().unwrap(), },
					str : STR { value: record[12].parse().unwrap(), },
					vit : VIT { value: record[13].parse().unwrap(), },
					int : INT { value: record[14].parse().unwrap(), },
					men : MEN { value: record[15].parse().unwrap(), },
					agi : AGI { value: record[16].parse().unwrap(), },
					dex : DEX { value: record[17].parse().unwrap(), },
					luk : LUK { value: record[18].parse().unwrap(), },
					unit_sprite : UnitSprite { value: record[19].to_string(), },
					dir: DIR { direction: Direction::from_string(record[20].to_string()), },
					movement_range: MovementRange { value: record[21].parse().unwrap(), },
					attack_range: AttackRange { value: record[22].parse().unwrap(), },
					attack_type: AttackType::from_string(record[23].to_string()),
				},
				Unit,
			));
		}
		events2.send(UnitsGeneratedEvent);
	}
}

// Server
fn place_units_on_map_system(mut events: EventReader<UnitsGeneratedEvent>, unit_positions: Query<(&UnitId, &PosX, &PosY)>, mut tiles: Query<(&Tile, &Pos, &mut Text)>, mut commands: Commands, mut next_state: ResMut<NextState<GameState>>) {
	
	for event in events.iter() {
		info!("DEBUG: Starting to place units on map...");
		// For each Unit...
		for (unit_id, unit_position_x, unit_position_y) in unit_positions.iter() {
			// Get the unit X and Y coordinates.
			let x = unit_position_x.value;
			let y = unit_position_y.value;
			
			// Get the tile at coordinates (x, y)
			for (tile, pos, mut text) in tiles.iter_mut() {
				if pos.x == x && pos.y == y {
					// Assign unit ID to tile.
					info!("DEBUG: Assigning unit ID to tile.");
					text.sections[0].value = unit_id.value.to_string();
				}
			}
		}
		info!("DEBUG: Finished placing units on map.");
		
		//info!("DEBUG: Setting GameState to Wait...");
		////commands.insert_resource(NextState(GameState::Wait));
		//next_state.set(GameState::Wait);	
		//info!("DEBUG: Set GameState to Wait.");
		
		info!("DEBUG: Setting GameState to LoadingComplete...");
		//commands.insert_resource(NextState(GameState::Wait));
		next_state.set(GameState::LoadingComplete);	
		info!("DEBUG: Set GameState to LoadingComplete.");
	}
}

// Client
fn init_cursor_system(mut commands: Commands, asset_server: Res<AssetServer>,) {
	commands.spawn((
		Cursor {
			x: 0,
			y: 0,
		},
		SpriteBundle {
			texture: asset_server.load("cursor.png"),
			visibility: Visibility::Hidden,
			..default()
		}));
}

// Client & Server
fn start_game_system(mut input: ResMut<Input<KeyCode>>, mut events: EventWriter<GameStartEvent>, mut commands: Commands, mut next_state: ResMut<NextState<GameState>>) {
	if input.just_pressed(KeyCode::Space) {
		info!("DEBUG: Setting GameState to Loading...");
		//commands.insert_resource(NextState(GameState::Loading));
		next_state.set(GameState::Loading);
		info!("DEBUG: Set GameState to Loading.");
        events.send(GameStartEvent);
    } 
}

// Client
fn setup_cursor_system(mut commands: Commands, mut tiles: Query<(&Tile, &Pos, &mut Text)>, game: Res<Game>, units: Query<(&UnitId, &PosX, &PosY)>, mut cursors: Query<&mut Cursor>) {
	
	info!("DEBUG: setup_cursor_system running...");
	// Setup cursor.
	
	for (unit, pos_x, pos_y) in units.iter() {
		if unit.value == game.current_unit {
			for (tile, pos, mut text) in tiles.iter_mut() {
				if pos.x == pos_x.value && pos.y == pos_y.value {
					// Place cursor at the current unit position.
					info!("DEBUG: Current unit is {}, at coordinates ({}, {}). Placing cursor there.", unit.value, pos_x.value, pos_y.value);
					
					// Build cursor string.
					let mut cursor = "[".to_owned();
					cursor.push_str(&text.sections[0].value);
					cursor.push_str("]");
					
					// Assign cursor string to map.
					text.sections[0].value = cursor;
					
					// Update cursor entity.
					for mut _cursor in cursors.iter_mut() {
						_cursor.x = pos.x;
						_cursor.y = pos.y;
					}
				}
			}
		}
	}
}

// Client
fn setup_cursor_system_2(
mut commands: Commands,
mut tiles: Query<(Entity, &Pos), With<GameText>>,
game: Res<Game>,
units: Query<(&UnitId, &Pos)>,
mut cursor_query: Query<(Entity, &mut Cursor, &mut Visibility)>,
asset_server: Res<AssetServer>,
) {
	
	info!("DEBUG: setup_cursor_system_2 running...");
	// Setup cursor.
	let (entity, mut cursor, mut visibility) = cursor_query.single_mut();
	
	// Find the tile the current unit is on.
	for (unit_id, pos) in units.iter() {
		if unit_id.value == game.current_unit {
			// Set the cursor to the current unit tile.
			cursor.x = pos.x;
			cursor.y = pos.y;
			
			// Make the cursor sprite visible.
			*visibility = Visibility::Visible;
			
		}
	}
}

// Prototype
fn move_cursor_2(
map_query: Query<&Map>,
mut cursor_query: Query<(&mut Cursor)>,
mut input: ResMut<Input<KeyCode>>,
) {
	let mut cursor = cursor_query.single_mut();
	let map = &map_query.single().map;
	if input.just_pressed(KeyCode::W) {
		if cursor.y == map[0].len() - 1 {
			info!("DEBUG: You can't move the cursor there.");
		} else {		
			// Move Cursor North.
			cursor.y += 1;
		}
	}

	if input.just_pressed(KeyCode::A) {
		
		if cursor.x == 0 {
			info!("DEBUG: You can't move the cursor there.");
		} else {		
			// Move Cursor West.
			cursor.x -= 1;
		}
	}
	
	if input.just_pressed(KeyCode::S) {
		if cursor.y == 0 {
			info!("DEBUG: You can't move the cursor there.");
		} else {		
			// Move Cursor South.
			cursor.y -= 1;
		}
	}
	
	if input.just_pressed(KeyCode::D) {
		if cursor.x == map.len() - 1 {
			info!("DEBUG: You can't move the cursor there.");
		} else {		
			// Move Cursor East.
			cursor.x += 1;
		}
	}
}

// Prototype
fn position_cursor(
map_query: Query<&Map>,
mut cursor_query: Query<(&Cursor, &mut Transform), Without<GameText>>,
tiles_query: Query<&Transform, With<GameText>>,
) {
	// Get the cursor.
	let (cursor, mut transform) = cursor_query.single_mut();
	
	// Get the map.
	let map = &map_query.single().map;
	
	// Get the transform of the tile the unit is on.
	let tile_entity = map[cursor.x][cursor.y].3[map[cursor.x][cursor.y].3.len() - 1];
	if let Ok(tile_transform) = tiles_query.get(tile_entity) {
		// Set the transform of the cursor to be the same as the tile's transform.
		transform.translation = tile_transform.translation.clone();
		
		// Add the required Y modifier to ensure the correct placement
		// at the center of the tile.
		transform.translation.y += 16.0;
		// Add the required Z modifier to ensure the correct Z-ordering.
		// This modifier is equal to half the unit's Z modifier,
		// so that the cursor will be placed above the tile but below the unit.
		transform.translation.z += 0.000000005;
	}
}

fn hide_cursor(
mut commands: Commands,
mut cursor_query: Query<(Entity, &mut Visibility), With<Cursor>>,
) {
	let (entity, mut visibility) = cursor_query.single();
	
	visibility = &Visibility::Hidden;
}

// Client
fn wait_turn_system(mut units: Query<(Entity, &mut WTCurrent, &WTMax, &UnitId, &UnitTeam)>, mut game: ResMut<Game>, mut commands: Commands, mut next_state: ResMut<NextState<TurnState>>) {
	
	// Decrease all units WT. If WT equals 0, set the unit as the current unit turn.
	for (entity, mut wt_current, wt_max, unit_id, unit_team) in units.iter_mut() {
		if wt_current.value == 0 {
			info!("DEBUG: Unit with UnitId {} has WTCurrent of 0.", unit_id.value);
		
			game.current_unit = unit_id.value;
			info!("DEBUG: It is now unit {} turn.", unit_id.value);
			
			game.current_team = unit_team.value;
			info!("DEBUG: It is now team {} turn.", unit_team.value);
			
			commands.entity(entity).insert(CurrentUnit {});
			
			// Find if team is controlled by player.
			if let Some(player) = game.players.get(&game.current_team) {
				match player {
					ControlledBy::Player => {
						// Player turn.
						// Set TurnState to Turn.
						info!("DEBUG: It is now the player's turn.");
						info!("DEBUG: Setting TurnState to Turn...");
						next_state.set(TurnState::Turn);
						info!("DEBUG: Set TurnState to Turn.");
					},
					ControlledBy::AI => {
						// AI turn.
						// Set TurnState to AI.
						info!("DEBUG: It is now the AI's turn.");
						info!("DEBUG: Setting TurnState to AI...");
						next_state.set(TurnState::AI);
						info!("DEBUG: Set TurnState to AI.");
					},
					ControlledBy::None => {
						info!("DEBUG: There isn't any player or AI controlling the current team. Please assign a controller to this team.");
					}
				}
			}
		} else {
			wt_current.value = wt_current.value - 1;
		}
	}
}

// Client
fn end_turn_single_player(mut input: ResMut<Input<KeyCode>>, mut units: Query<(Entity, &mut WTCurrent, &WTMax), With<CurrentUnit>>, mut commands: Commands, mut next_state: ResMut<NextState<TurnState>>) {
	if input.just_pressed(KeyCode::T) {
		info!("DEBUG: The current unit has ended its turn.");
		info!("DEBUG: Reseting the unit's WT.");
		for (entity, mut wt_current, wt_max) in units.iter_mut() {
			if wt_current.value == 0 {
				wt_current.value = wt_max.value;
				commands.entity(entity).remove::<CurrentUnit>();
				break;
			}
		}
		
		// Set TurnState to Wait.
		info!("DEBUG: Setting TurnState to Wait...");
		next_state.set(TurnState::Wait);
		info!("DEBUG: Set TurnState to Wait.");
	}
}

// Client
fn end_turn_system(mut input: ResMut<Input<KeyCode>>, mut units: Query<(&mut WTCurrent, &WTMax)>, mut commands: Commands, mut client: ResMut<Client>) {
	if input.just_pressed(KeyCode::T) {
		//info!("DEBUG: The current unit has ended its turn.");
		//info!("DEBUG: Reseting the unit's WT.");
		//for (mut wt_current, wt_max) in units.iter_mut() {
			//if wt_current.value == 0 {
				//wt_current.value = wt_max.value;
				//break;
			//}
		//}
		
		info!("DEBUG: Sending Wait message...");
		client
			.connection()
			.try_send_message(ClientMessage::Wait);
		info!("DEBUG: Sent Wait message.");
		
		//// Set GameState to Wait.
		//info!("DEBUG: Setting GameState to Wait...");
		//commands.insert_resource(NextState(GameState::Wait));
		//info!("DEBUG: Set GameState to Wait.");
	}
}

// Client
fn setup_game_resource_system(mut commands: Commands) {
	let mut players = HashMap::new();
	players.insert(1, ControlledBy::Player);
	players.insert(2, ControlledBy::AI);
	
	commands.insert_resource(Game {
		current_unit: 0,
		current_team: 1,
		players: players,
		winner: ControlledBy::None,
		is_multiplayer: false,
	});
}

// Client
fn setup_game_resource_system_multiplayer(mut commands: Commands) {
	let mut players = HashMap::new();
	players.insert(1, ControlledBy::Player);
	players.insert(2, ControlledBy::Player);
	
	commands.insert_resource(Game {
		current_unit: 0,
		current_team: 1,
		players: players,
		winner: ControlledBy::None,
		is_multiplayer: true,
	});
}

// Client
fn start_connection(mut client: ResMut<Client>) {
	client
		.open_connection(
			ConnectionConfiguration::from_strings(
				//"127.0.0.1:6000",
				//"139.162.244.70:6000",
				"178.79.171.209:6000",
                "0.0.0.0:0",
            ).unwrap(),
            CertificateVerificationMode::SkipVerification,
        ).unwrap();
}

// Client
fn send_get_client_id_message(client: ResMut<Client>) {
	info!("DEUBG: Sending GetClientId message...");
	client
		.connection()
		.try_send_message(ClientMessage::GetClientId);
	info!("DEUBG: Sent GetClientId message.");
}

// Client
fn handle_server_messages(
    mut client: ResMut<Client>,
    mut events: EventWriter<GameStartEvent>,
    mut commands: Commands,
    mut client_data: ResMut<ClientData>,
    mut game: ResMut<Game>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    while let Ok(Some(message)) = client.connection_mut().receive_message::<ServerMessage>() {
        match message {
            ServerMessage::StartGame { client_id } => { 
				info!("DEBUG: Server has sent StartGame message.");
				
				
				// Start game.
				info!("DEBUG: Starting game...");
				info!("DEBUG: Setting GameState to Loading...");
				//commands.insert_resource(NextState(GameState::Loading));
				next_state.set(GameState::Loading);
				info!("DEBUG: Set GameState to Loading.");
				events.send(GameStartEvent);
				
				// Set `Game` Resource `is_multiplayer` to true.
				game.is_multiplayer = true;
			},
			ServerMessage::ClientId { client_id } => {
				// Configure ClientId.
				client_data.client_id = client_id;
				info!("DEBUG: Client ID is now {}.", client_data.client_id);
			},
			_ => { empty_system(); },
        }
    }
}

// Client
fn send_start_game_message_system(mut input: ResMut<Input<KeyCode>>, client: Res<Client>, mut next_state: ResMut<NextState<GameState>>) {
	if input.just_pressed(KeyCode::Space) {
		client
			.connection()
			.try_send_message(ClientMessage::StartGame);
	} else if input.just_pressed(KeyCode::M) {
		next_state.set(GameState::LoadAmbush);
	}
}

// Client
fn handle_player_turn_server_message(
	mut client: ResMut<Client>,
	mut commands: Commands,
	client_data: Res<ClientData>,
	mut units: Query<(Entity, &UnitId, &mut WTCurrent, &mut UnitActions)>,
//	mut current_unit_query: Query<(Entity, &mut UnitActions), With<CurrentUnit>>,
	mut game: ResMut<Game>,
	map_query: Query<&mut Map>,
	mut next_state: ResMut<NextState<GameState>>,
	mut next_turn_state: ResMut<NextState<TurnState>>,
	state: Res<State<GameState>>,
) {
	let mut map: Vec<Vec<(usize, TileType, Vec<Entity>, Vec<Entity>)>> = Vec::new();
	match state.get() {
		GameState::MainMenu => {
			empty_system();
		},
		_ => {
			map = map_query.single().map.clone();
		}
	}  

	while let Ok(Some(message)) = client.connection_mut().receive_message::<ServerMessage>() {
		match message {
			ServerMessage::PlayerTurn { client_id, current_unit } => {
				info!("DEBUG: Received PlayerTurn message.");
				info!("DEBUG: Current state is {:?}.", state.get());
				// Update Game resouce.
				info!("DEBUG: Setting current unit to {}.", current_unit);
				game.current_unit = current_unit;
				info!("DEBUG: Set current unit to {}.", game.current_unit);
				
				// Assign the `CurrentUnit` component to the current unit.
				for (entity, unit_id, mut current_wt, mut unit_actions) in units.iter_mut() {
					if unit_id.value == game.current_unit {
						info!("DEBUG: Inserting `CurrentUnit` component into unit {}.", unit_id.value);
						commands.entity(entity).insert(CurrentUnit {});
					}
				}
				
				if client_id == client_data.client_id {
					// Set state to Battle.
					info!("DEBUG: Setting GameState to Battle...");
					//commands.insert_resource(NextState(GameState::Battle));
					let current_state = state.get();
					info!("DEBUG: Current state is {:?}.", current_state);
					//match current_state {
					//	Some(state) => {
					//		info!("DEBUG: Current state is {:?}.", state);
					//	},
					//	None => {
					//		info!("DEBUG: No current state.");
					//	}
					//}
					next_state.set(GameState::Battle);
					info!("DEBUG: Set GameState to Battle.");
					
					// Set `TurnState` to `Turn`.
					info!("DEBUG: Setting TurnState to Turn...");
					next_turn_state.set(TurnState::Turn);
					info!("DEBUG: Set TurnState to Turn.");
				} else {
					// Set state to Battle.
					info!("DEBUG: Setting GameState to Battle...");
					next_state.set(GameState::Battle);
					info!("DEBUG: Set GameState to Battle.");
					//info!("DEBUG: Current state is {:?}.", state.get());
					
					// Set `TurnState` to `Wait`.
					info!("DEBUG: Setting TurnState to Wait...");
					next_turn_state.set(TurnState::Wait);
					info!("DEBUG: Set TurnState to Wait.");
				}
			},
			ServerMessage::WaitTurn { wait_turns } => {
				info!("DEBUG: Received WaitTurn message.");
				
				// Update unit WTs.
				for unit_wt in wait_turns {
					for (entity, unit_id, mut current_wt, mut unit_actions) in units.iter_mut() {
						if unit_id.value == unit_wt.0.value {
							current_wt.value = unit_wt.1.value;
							//break;
						}
					}
				}
				
				//// Set state to WaitTurn.
				//info!("DEBUG: Setting GameState to WaitTurn...");
				//commands.insert_resource(NextState(GameState::WaitTurn));
				//info!("DEBUG: Set GameState to WaitTurn.");
			},
			ServerMessage::Wait => {
				// Set state to Wait.
				info!("DEBUG: Received Wait message.");
				
				// Remove the `CurrentUnit` component from the current unit.
				for (entity, unit_id, mut current_wt, mut unit_actions) in units.iter_mut() {
					if unit_id.value == game.current_unit {
						info!("DEBUG: Removing `CurrentUnit` component from unit.");
						commands.entity(entity).remove::<CurrentUnit>();
					}
				}
				
				info!("DEBUG: Setting GameState to Wait...");
				//commands.insert_resource(NextState(GameState::Wait));
				next_state.set(GameState::Wait);
				info!("DEBUG: Set GameState to Wait.");
				info!("DEBUG: Current state is {:?}.", state.get());
				
				info!("DEBUG: Setting TurnState to Wait...");
				next_turn_state.set(TurnState::Wait);
				info!("DEBUG: Set TurnState to Wait.");
			},
			ServerMessage::Move { origin, destination } => {
				info!("DEBUG: Received `Move` message from server.");
				
				// Insert `Move` `UnitAction` into current unit.
				info!("DEBUG: Inserting `Move` `UnitAction` into current unit...");
				for (entity, unit_id, mut current_wt, mut unit_actions) in units.iter_mut() {
					if unit_id.value == game.current_unit {
						unit_actions.unit_actions.push(UnitActionTuple(UnitAction::Move {
							origin: Pos { x: origin.x, y: origin.y, },
							destination: Pos { x: destination.x, y: destination.y },
							timer: Timer::from_seconds(4.0, TimerMode::Once),
						}, 0.0));
						info!("DEBUG: Finished inserting `Move` `UnitAction` into current unit.");
					}
				}
			},
			ServerMessage::BasicAttack { attacker, target, damage, is_counterattack } => {
				info!("DEBUG: Received `BasicAttack` message from server.");
				
				// Insert `BasicAttack` `UnitAction` into current unit.
				info!("DEBUG: Inserting `BasicAttack` `UnitAction` into current unit...");
				if let Ok((entity, unit_id, mut current_wt, mut unit_actions)) = units.get_mut(map[attacker.x][attacker.y].2[map[attacker.x][attacker.y].2.len() - 1]) {
					unit_actions.unit_actions.push(UnitActionTuple(UnitAction::BasicAttack {
						target: Pos { x: target.x, y: target.y, },
						is_counterattack: is_counterattack,
						damage: damage.clone(),
					}, 0.0));
					info!("DEBUG: Finished inserting `BasicAttack` `UnitAction` into current unit.");
				}
				
				// Get the attacker and target entities from map.
				let entity = map[attacker.x][attacker.y].2[map[attacker.x][attacker.y].2.len() - 1];
				let target_entity = map[target.x][target.y].2[map[target.x][target.y].2.len() - 1];
				
				// Insert an `Attacker` marker component on the attacking unit.
				commands.entity(entity).insert(Attacker {});
				
				// Insert the `Target` marker component on the target unit.
				commands.entity(target_entity).insert(Target {});
								
				// Remove the AttackTiles component from the unit.
				commands.entity(entity).remove::<AttackTiles>();
				
				// Set State
				next_turn_state.set(TurnState::Turn);
				
				
			},
			ServerMessage::GameOver { winner } => {
				info!("DEBUG: Battle is over.");
				info!("DEBUG: Winner is: {:?}.", winner);
				
				info!("DEBUG: Setting GameState to MainMenu...");
				next_state.set(GameState::MainMenu);
				info!("DEBUG: Set GameState to MainMenu.");
				info!("DEBUG: Setting TurnState to Wait...");
				next_turn_state.set(TurnState::Wait);
				info!("DEBUG: Set TurnState to Wait.");
			},
			_ => { empty_system(); },
		}
	}
}

fn handle_wait_turn_server_message(
	mut client: ResMut<Client>,
	mut commands: Commands,
	mut units: Query<(&UnitId, &mut WTCurrent)>, 
) {
	while let Ok(Some(message)) = client.connection_mut().receive_message::<ServerMessage>() {
		match message {
			ServerMessage::WaitTurn { wait_turns } => {
				info!("DEBUG: Received WaitTurn message.");
				
				// Update unit WTs.
				for unit_wt in wait_turns {
					for (unit_id, mut current_wt) in units.iter_mut() {
						if unit_id.value == unit_wt.0.value {
							current_wt.value = unit_wt.1.value;
							break;
						}
					}
				}
				
				//// Set state to WaitTurn.
				//info!("DEBUG: Setting GameState to WaitTurn...");
				//commands.insert_resource(NextState(GameState::WaitTurn));
				//info!("DEBUG: Set GameState to WaitTurn.");
			},
			_ => { empty_system(); },
		}
	}
}

// Server
fn cursor_already_spawned(cursors: Query<&Cursor>) -> bool {
	let mut cursor_spawned = false;
	for cursor in cursors.iter() {
		cursor_spawned = true;
	}
	return cursor_spawned;
}

// Client
fn setup_grid_system(mut commands: Commands) {
	// Create map.
	info!("DEBUG: Creating map...");
	let mut map: Vec<Vec<(usize, TileType, Vec<Entity>, Vec<Entity>)>> = Vec::new();
	for i in 0..20 {
		let mut map_line: Vec<(usize, TileType, Vec<Entity>, Vec<Entity>)> = Vec::new();
		for j in 0..20 {
			map_line.push((1, TileType::Grass, Vec::new(), Vec::new()));
		}
		map.push(map_line);
	}
	
	//for i in 0..3 {
	//	map[i][0].0 = 10;
	//}
	
	info!("DEBUG: Created map.");
	
	commands.spawn((
		Map { map: map },
	));
}

// Client
fn setup_camera_system(mut commands: Commands) {
	info!("DEBUG: Spawning camera...");
	commands.spawn(Camera2dBundle::default());
	info!("DEBUG: Spawned camera.");
}

// Client
fn setup_text_system(mut query: Query<&mut Map>, mut commands: Commands, asset_server: Res<AssetServer>) {
	let font = asset_server.load("fonts/FiraSans-Bold.ttf");
	let text_style = TextStyle {
		font,
		font_size: 20.0,
		color: Color::WHITE,
	};
	let text_alignment = TextAlignment::Center;
	
	info!("DEBUG: Spawning tiles...");
	
	let mut map = query.single_mut();
	for i in 0..map.map.len() {
		for j in 0..map.map[i].len() {
			//let tile_string: String = map.map[i][j].2.to_string();
			
			for k in 0..map.map[i][j].0 {
				let entity_id = commands.spawn((
					//Text2dBundle {
					//	text: Text::from_section(tile_string, text_style.clone()).with_alignment(text_alignment),
					//	transform: Transform::from_xyz((i as f32) * 256.0 / 2.0 - (j as f32) * 256.0 / 2.0, (i as f32) * 128.0 / 2.0 + (j as f32) * 128.0 / 2.0, 0.0),
					//	..default()
					//},
					SpriteBundle {
						texture: asset_server.load("tile.png"),
						transform: Transform::from_xyz((i as f32) * 256.0 / 2.0 - (j as f32) * 256.0 / 2.0, ((i as f32) * 128.0 / 2.0 + (j as f32) * 128.0 / 2.0) * (111.0 / (128.0 / 2.0) - 1.0) + (k as f32) * 30.0, 1.0),
						..default()
					},
					GameText,
				)).id();
				
				map.map[i][j].3.push(entity_id);
			}
		}
	}
	
	info!("DEBUG: Spawned tiles.");
}

// Client
fn z_order_system(
mut query: Query<&mut Transform, With<GameText>>,
mut unit_query: Query<&mut Transform, (With<Unit>, Without<GameText>)>,
map_query: Query<&Map>,
) {
	//info!("DEBUG: Z-Order system running...");
	let map = &map_query.single().map;
	//info!("DEBUG: Map length is: {}.", map.len());
	
	let mut counter = 0.0;
	let mut counter_2 = 0.0;
	
	for i in (0..map.len()).rev() {
		for j in (0..map[i].len()).rev() {
			counter += 0.00001;
			
			//info!("Tile Entity ID is: {:?}.", map[i][j].3);
			for k in 0..map[i][j].3.len() {
				counter_2 += 0.0000001;
				if let Ok(mut tile_transform) = query.get_mut(map[i][j].3[k]) {
					//info!("DEBUG: Tile position is: {:?}.", tile_transform.translation);
					tile_transform.translation.z = counter + counter_2;
				}
			}
			
			// If there is a unit on the tile, order it.
			if map[i][j].2.len() > 0 {
				// Order unit.
				if let Ok(mut unit_transform) = unit_query.get_mut(map[i][j].2[0]) {
					if let Ok(mut tile_transform) = query.get_mut(map[i][j].3[map[i][j].3.len() - 1]) {
						//unit_transform.translation.y += 100.0;
					}
					//unit_transform.translation.z = counter + counter_2 + 0.0000001;
				}
			}
		}
	}
}

// Client
fn z_unit_order_system(
mut query: Query<&mut Transform, (With<GameText>, Without<Unit>)>,
mut unit_query: Query<(&mut Transform, &Pos), (With<Unit>, Without<GameText>)>,
map_query: Query<&Map>,
) {
	let map = &map_query.single().map;

	for (mut unit_transform, pos) in unit_query.iter_mut() {
		// Get the tile the unit is on.
		//map[pos.x][pos.y]
		
		if let Ok(mut tile_transform) = query.get(map[pos.x][pos.y].3[map[pos.x][pos.y].3.len() - 1]) {
			unit_transform.translation.z = tile_transform.translation.z + 0.00000001;
		}
	}
}

// Client
fn move_camera_system(
mut camera_transform_query: Query<&mut Transform, With<Camera>>,
windows: Query<&Window, With<PrimaryWindow>>,
) {
	let window = windows.single();
	
	let mut camera_transform = camera_transform_query.single_mut();
	
	if let Some(_position) = window.cursor_position() {
		// Cursor is inside the window.
		//info!("DEBUG: Cursor position is: {:?}.", _position);
		//info!("DEBUG: Window width is: {:?}.", window.width());
		//info!("DEBUG: Window height is: {:?}.", window.height());
		
		if _position.x <= 0.0 + 20.0 {
			//info!("DEBUG: Camera translation is: {:?}.", camera_transform.translation);
			camera_transform.translation -= Vec3::X * 3.0;
		} else if _position.x >= window.width() - 20.0 {
			camera_transform.translation += Vec3::X * 3.0;
		}
		
		if _position.y <= 0.0 + 20.0 {
			//info!("DEBUG: Camera translation is: {:?}.", camera_transform.translation);
			camera_transform.translation += Vec3::Y * 3.0;
		} else if _position.y >= window.height() - 20.0 {
			camera_transform.translation -= Vec3::Y * 3.0;
		}
		
	} else {
		// Cursor is not inside the window.
	}
}

// Prototype
fn spawn_naked_swordsman(
mut commands: Commands,
mut map_query: Query<&mut Map>,
asset_server: Res<AssetServer>,
) {
	let mut map = &mut map_query.single_mut().map;
	
	info!("DEBUG: Spawning Naked Swordsman...");
	
	let entity_id = commands.spawn((
		SpriteBundle {
			texture: asset_server.load("naked_fanatic_swordsman_east.png"),
			transform: Transform::from_xyz((4 as f32) * 256.0 / 2.0 - (4 as f32) * 256.0 / 2.0, (4 as f32) * 128.0 / 2.0 + (4 as f32) * 128.0 / 2.0 + (map[4][4].0 as f32) * 15.0 + 100.0, 1.0),
			..default()
		},
		Unit,
		Pos {
			x: 4,
			y: 4,
		},
		UnitActions { unit_actions: Default::default(), processing_unit_action: false, },
		NakedSwordsman {},
	)).id();
	
	map[4][4].2.push(entity_id);
	
	info!("DEBUG: Spawned Naked Swordsman.");
}

// Prototype
fn move_gaul_warrior(
mut map_query: Query<&mut Map>,
mut unit_query: Query<(Entity, &mut Pos, &mut Handle<Image>), With<NakedSwordsman>>,
mut input: ResMut<Input<KeyCode>>,
mut asset_server: Res<AssetServer>,
) {
	let mut map = &mut map_query.single_mut().map;
	
	if input.just_pressed(KeyCode::W) {
		info!("DEBUG: W pressed.");
		//info!("DEBUG: unit_query length is: {}.", unit_query.iter_mut().len());
		for (entity_id, mut pos, mut sprite) in unit_query.iter_mut() {
			if pos.y == (map.len() - 1) {
				info!("DEBUG: You can't move there.");
			} else {
				// Change unit sprite to face north.
				//info!("DEBUG: Sprite is: {:?}.", sprite);
				*sprite = asset_server.load("naked_fanatic_swordsman_north.png");
				map[pos.x][pos.y + 1].2.push(entity_id);
				map[pos.x][pos.y].2.pop();
				pos.y += 1;
			}
		}
	}
	
	if input.just_pressed(KeyCode::S) {
		info!("DEBUG: S pressed.");
		//info!("DEBUG: unit_query length is: {}.", unit_query.iter_mut().len());
		for (entity_id, mut pos, mut sprite) in unit_query.iter_mut() {
			if pos.y == 0 {
				info!("DEBUG: You can't move there.");
			} else {
				// Change unit sprite to face south.
				//info!("DEBUG: Sprite is: {:?}.", sprite);
				*sprite = asset_server.load("naked_fanatic_swordsman_south.png");
				map[pos.x][pos.y - 1].2.push(entity_id);
				map[pos.x][pos.y].2.pop();
				pos.y -= 1;
			}
		}
	} 
	
	if input.just_pressed(KeyCode::D) {
		info!("DEBUG: D pressed.");
		//info!("DEBUG: unit_query length is: {}.", unit_query.iter_mut().len());
		for (entity_id, mut pos, mut sprite) in unit_query.iter_mut() {
			if pos.x == (map.len() - 1) {
				info!("DEBUG: You can't move there.");
			} else {
				// Change unit sprite to face east.
				//info!("DEBUG: Sprite is: {:?}.", sprite);
				*sprite = asset_server.load("naked_fanatic_swordsman_east.png");
				map[pos.x + 1][pos.y].2.push(entity_id);
				map[pos.x][pos.y].2.pop();
				pos.x += 1;
			}
		}
	} 
	
	if input.just_pressed(KeyCode::A) {
		info!("DEBUG: A pressed.");
		//info!("DEBUG: unit_query length is: {}.", unit_query.iter_mut().len());
		for (entity_id, mut pos, mut sprite) in unit_query.iter_mut() {
			if pos.x == 0 {
				info!("DEBUG: You can't move there.");
			} else {
				// Change unit sprite to face south.
				//info!("DEBUG: Sprite is: {:?}.", sprite);
				*sprite = asset_server.load("naked_fanatic_swordsman_west.png");
				map[pos.x - 1][pos.y].2.push(entity_id);
				map[pos.x][pos.y].2.pop();
				pos.x -= 1;
			}
		}
	} 
}

// Client & Server
fn grid_already_setup(query: Query<&Map>) -> bool {
	if query.iter().len() == 0 {
		return false;
	} else {
		return true;
	}
}

// Client
fn text_already_setup(query: Query<&GameText>) -> bool {
	if query.iter().len() == 0 {
		return false;
	} else {
		return true;
	}
}

// Prototype
fn center_camera_on_unit(
unit_transform_query: Query<&Transform, With<CurrentUnit>>,
mut camera_transform_query: Query<&mut Transform, (With<Camera>, Without<CurrentUnit>)>,
) {
	let unit_transform = unit_transform_query.single();
	let mut camera_transform = camera_transform_query.single_mut();
	
	camera_transform.translation = Vec3::new(unit_transform.translation.x, unit_transform.translation.y, camera_transform.translation.z);
}

fn test_ortho_projection(
ortho_projection_query: Query<&OrthographicProjection>,
) {
	let ortho_projection = ortho_projection_query.single();
	
	info!("DEBUG: Orthographic projection far is: {:?}.", ortho_projection.far);
}


// Prototype
fn process_unit_actions(
mut commands: Commands,
mut unit_actions_query: Query<(Entity, &mut UnitActions)>,
mut map_query: Query<&mut Map>,
time: Res<Time>,
) {
	let map = &map_query.single_mut().map;
	
	//info!("DEBUG: unit_actions_query length is: {}.", unit_actions_query.iter().len());
	
	for (entity, mut unit_actions) in unit_actions_query.iter_mut() {
		
		if unit_actions.unit_actions.len() == 0 {
			continue;
		} else if unit_actions.processing_unit_action == true {
			continue;
		} else if time.elapsed_seconds() < unit_actions.unit_actions[0].1 {
			continue;
		} else if unit_actions.unit_actions[0].1 == 0.0 || time.elapsed_seconds() >= unit_actions.unit_actions[0].1 {
			unit_actions.processing_unit_action = true;
			
			let current_unit_action = &unit_actions.unit_actions[0].0;
		
			match current_unit_action {
				UnitAction::Move { origin, destination, timer, } => {
					info!("DEBUG: Current unit action is Move.");
					commands.entity(entity).insert(MoveAction { 
						origin: origin.clone(),
						destination: destination.clone(),
						timer: timer.clone(),
					});
				},
				UnitAction::Talk { message } => {
					info!("DEBUG: Current unit action is Talk.");
					commands.entity(entity).insert(TalkAction { message: message.clone(), });
				},
				UnitAction::BasicAttack { target, is_counterattack, damage } => {
					info!("DEBUG: Current unit action is BasicAttack.");
					commands.entity(entity).insert(BasicAttackAction { target: target.clone(), is_counterattack: is_counterattack.clone(), damage: damage.clone(), });
				}
				UnitAction::DoNothing => {
					info!("DEBUG: Current unit action is DoNothing.");
					commands.entity(entity).insert(DoNothingAction);
				}
			}
		}
	}
}

// Prototype
fn process_move_actions(
mut commands: Commands,
mut map_query: Query<&mut Map>,
mut unit_query: Query<(Entity, &mut UnitActions, &mut Pos, &MoveAction, &mut MoveActions)>,
mut next_state: ResMut<NextState<GameState>>,
) {
	let map = &mut map_query.single_mut().map;
	
	for (entity, mut unit_actions, mut pos, move_action, mut move_actions) in unit_query.iter_mut() {
		info!("DEBUG: Processing MoveAction...");
		info!("DEBUG: Move destination is: {}, {}.", move_action.destination.x, move_action.destination.y);
		
		// Calculate path.
		let path = find_path(map.to_vec(), move_action.origin, move_action.destination);
		if let Some(mut path) = path {
			
			let origin_backup: Pos = path[0];
			
			// Remove the first entry because it is the tile the unit is on.
			path.remove(0);
			
			for i in 0..path.len() {
				info!("DEBUG: Path step is {}, {}.", path[i].x, path[i].y);
				// Create MoveAction
				if i == 0 {
					let new_move_action = MoveAction { origin: origin_backup, destination: path[i], timer: Timer::from_seconds(2.0, TimerMode::Once)};
					// Add MoveAction to MoveActions component.
					move_actions.move_actions.push(new_move_action);
				} else {
					let new_move_action = MoveAction { origin: path[i - 1], destination: path[i], timer: Timer::from_seconds(2.0, TimerMode::Once)};
					// Add MoveAction to MoveActions component.
					move_actions.move_actions.push(new_move_action);
				}
			}
		}
		
		// Set GameState to Move
		info!("DEBUG: Setting GameState to Move...");
		next_state.set(GameState::Move);
		info!("DEBUG: Set GameState to Move.");
		
		break;
	}
}

// Prototype
fn handle_move_state(
mut commands: Commands,
mut map_query: Query<&mut Map>,
mut unit_query: Query<(Entity, &mut Transform, &mut UnitActions, &mut Pos, &MoveAction, &mut MoveActions, &mut DIR), Without<GameText>>,
tile_transform_query: Query<&Transform, (With<GameText>, Without<Unit>)>,
mut next_state: ResMut<NextState<GameState>>,
mut next_turn_state: ResMut<NextState<TurnState>>,
game: Res<Game>,
time: Res<Time>,
) {
	let mut map_component = map_query.single_mut();
	let mut map = &mut map_component.map;

	if unit_query.iter_mut().len() == 0 {
		if !game.is_multiplayer {
			// Game is in Single-Player mode.
			info!("DEBUG: No MoveActions remaining. Setting GameState to Ambush.");
			next_state.set(GameState::Ambush);
			info!("DEBUG: No MoveActions remaining. Set GameState to Ambush.");
			// Set TurnState to Turn.
			info!("DEBUG: Setting TurnState to Turn...");
			next_turn_state.set(TurnState::Turn);
			info!("DEBUG: Set TurnState to Turn.");
		} else {
			// Game is in Multiplayer mode.
			info!("DEBUG: No MoveActions remaining. Setting GameState to Battle.");
			next_state.set(GameState::Battle);
			info!("DEBUG: No MoveActions remaining. Set GameState to Battle.");
			// Set TurnState to Turn.
			info!("DEBUG: Setting TurnState to Turn...");
			next_turn_state.set(TurnState::Turn);
			info!("DEBUG: Set TurnState to Turn.");
		}
	} else {
	
		for (entity, mut transform, mut unit_actions, mut pos, move_action_component, mut move_actions, mut dir) in unit_query.iter_mut() {
			
			if move_actions.move_actions.len() == 0 {
				// This unit has completed its movement.
				unit_actions.processing_unit_action = false;
				unit_actions.unit_actions.remove(0);
				commands.entity(entity).remove::<MoveAction>();
				info!("DEBUG: Processed MoveAction.");
			} else {
				let move_action = &move_actions.move_actions[0];
				
				if map[move_action.destination.x][move_action.destination.y].2.len() > 0 {
					info!("DEBUG: Couldn't move unit. There's an unit already there.");
					unit_actions.processing_unit_action = false;
					unit_actions.unit_actions.remove(0);
					commands.entity(entity).remove::<MoveAction>();
					move_actions.move_actions.truncate(0);
					info!("DEBUG: Processed MoveAction.");
				} else {
					if move_action.timer.just_finished() {
						// Complete processing of MoveAction.
						
						info!("DEBUG: Completing processing of MoveAction...");
						map[move_action.destination.x][move_action.destination.y].2.push(entity);
						map[pos.x][pos.y].2.pop();
						
						pos.x = move_action.destination.x;
						pos.y = move_action.destination.y;
						
						
						
						move_actions.move_actions.remove(0);
						info!("DEBUG: Processed MoveAction.");
					
					} else {
						// Move is still in progress, update the unit's position gradually
						
						//info!("DEBUG: Move in progress, updating unit position.");
						//move_action.timer.tick(time.delta());
											
						
						let start_pos = &move_action.origin;
						let end_pos = &move_action.destination;
						
						// Set the unit's direction.
						if end_pos.x < start_pos.x {
							dir.direction = Direction::West;
						} else if end_pos.x > start_pos.x {
							dir.direction = Direction::East;
						}
						if end_pos.y < start_pos.y {
							dir.direction = Direction::South;
						} else if end_pos.y > start_pos.y {
							dir.direction = Direction::North;
						}
						
						
						let progress = move_action.timer.elapsed_secs() / 2.0;

						// Get target tile transform.
						let target_tile_entity = map[end_pos.x][end_pos.y].3[map[end_pos.x][end_pos.y].3.len() - 1];
						
						let target_tile_transform = tile_transform_query.get(target_tile_entity).unwrap();
						
						transform.translation.x += ((target_tile_transform.translation.x - transform.translation.x) * progress);
						transform.translation.y += ((target_tile_transform.translation.y - transform.translation.y  + 100.0) * progress);
						


						
	//					// To fix bug with z-ordering when the unit is moving.
	//					// ATTEMPT#1:
	//					transform.translation.z = target_tile_transform.translation.z + 0.00000001;

						// ATTEMPT#2:
						// The correct z-order depends on the tile the unit is moving to.
						if end_pos.x > start_pos.x {
							
						} else if end_pos.x < start_pos.x {
							transform.translation.z = target_tile_transform.translation.z  + 0.00000001;
						} else if end_pos.y < start_pos.y {
							transform.translation.z = target_tile_transform.translation.z  + 0.00000001;
						} else if end_pos.y > start_pos.y {
						
						}
						
					}
				}
			}
		}
	}
}

// Prototype
fn tick_move_timer(
mut move_actions_query: Query<&mut MoveActions, With<MoveAction>>,
time: Res<Time>
) {
	for (mut move_actions) in move_actions_query.iter_mut() {
		if move_actions.move_actions.len() != 0 {
			move_actions.move_actions[0].timer.tick(time.delta());
		}
	}
}

// Prototype
fn process_talk_actions(
mut commands: Commands,
mut unit_query: Query<(Entity, &mut UnitActions, &TalkAction)>,
asset_server: Res<AssetServer>,
mut next_state: ResMut<NextState<GameState>>,
) {
	
	
	for (entity, mut unit_actions, talk_action) in unit_query.iter_mut() {
		
		info!("DEBUG: Processing talk action...");
		
		info!("DEBUG: Setting GameState to Talk.");
		next_state.set(GameState::Talk);
		info!("DEBUG: Set GameState to Talk.");
		
		let portrait = asset_server.load("gaul_spearman_portrait.png");
		
		commands
			.spawn((NodeBundle {
				style: Style {
					width: Val::Percent(100.0),
					height: Val::Percent(20.0),
					position_type: PositionType::Absolute,
					top: Val::Percent(0.0),
					..Default::default()
				},
				background_color: BackgroundColor(Color::BLACK),
				..Default::default()
			},
			TalkUI {},
			))
			.with_children(|parent| {
			
				// Add the character portrait
				parent.spawn((ImageBundle {
					style: Style {
						width: Val::Percent(20.0),
						height: Val::Percent(100.0),
						..Default::default()
					},
					image: UiImage::new(portrait.clone()),
					..Default::default()
				}));
				
				// Add the message text
				parent.spawn((TextBundle {
					text: Text::from_section(
						talk_action.message.clone(),
						TextStyle {
							font: asset_server.load("fonts/FiraSans-Bold.ttf"),
							font_size: 20.0,
							color: Color::WHITE,
						},
					),
					..Default::default()
				}));
			});
		
	}
}

// Prototype
fn handle_talk_state(
mouse_button_input: Res<Input<MouseButton>>,
mut commands: Commands,
mut unit_query: Query<(Entity, &mut UnitActions, &TalkAction)>,
talk_ui_query: Query<(Entity, &TalkUI)>,
mut next_state: ResMut<NextState<GameState>>,
) {
	if mouse_button_input.just_pressed(MouseButton::Left) {
        info!("DEBUG: Left mouse button just pressed.");
        
        for (entity, mut unit_actions, talk_action) in unit_query.iter_mut() {
			
			// There should be only one unit with a `TalkAction`
			
			// Despawn the TalkUI
			let (ui_entity, talk_ui) = talk_ui_query.single();
			commands.entity(ui_entity).despawn();
			
			// Process unit action.
			unit_actions.unit_actions.remove(0);
			unit_actions.processing_unit_action = false;
			commands.entity(entity).remove::<TalkAction>();
			info!("DEBUG: Processed talk action.");
			
			// Set GameState back to Ambush.
			info!("DEBUG: Setting GameState to Ambush.");
			next_state.set(GameState::Ambush);
			info!("DEBUG: Set GameState to Ambush.");
        }
    }
}

// Prototype
fn process_do_nothing_actions(mut commands: Commands, mut unit_query: Query<(Entity, &mut UnitActions, &DoNothingAction)>) {
	for (entity, mut unit_actions, do_nothing_action) in unit_query.iter_mut() {
		info!("DEBUG: Processing DoNothing action...");
		
		unit_actions.unit_actions.remove(0);
		unit_actions.processing_unit_action = false;
		commands.entity(entity).remove::<DoNothingAction>();
		
		info!("DEBUG: Processed DoNothing action.");
	}	
}

// Prototype
fn process_basic_attack_actions(
mut commands: Commands,
map_query: Query<&Map>,
mut attack_unit_query: Query<(Entity, &UnitId, &mut UnitActions, &STR, &Pos, &mut DIR, &BasicAttackAction), (With<Attacker>, Without<Target>)>,
mut target_unit_query: Query<(&UnitId, &mut UnitActions, &Pos, &mut HPCurrent, &AttackRange, &AttackType), (With<Target>, Without<Attacker>)>,
game: Res<Game>,
) {
	let map = &map_query.single().map;
	
	//info!("DEBUG: attack_unit_query length is: {}.", attack_unit_query.iter().len());
	
	for (entity, unit_id, mut unit_actions, str, pos, mut dir, basic_attack_action) in attack_unit_query.iter_mut() {
		info!("DEBUG: Processing BasicAttack action...");
		
		// Get target entity from map.
		let target_entity = map[basic_attack_action.target.x][basic_attack_action.target.y].2[0];
		
		// Get target health.
		if let Ok((target_id, mut target_unit_actions, target_pos, mut hp_current, attack_range, attack_type)) = target_unit_query.get_mut(target_entity) {
			// Change attacker's direction to face the target.
			// Set the unit's direction.
			if target_pos.x < pos.x {
				dir.direction = Direction::West;
			} else if target_pos.x > pos.x {
				dir.direction = Direction::East;
			}
			if target_pos.y < pos.y {
				dir.direction = Direction::South;
			} else if target_pos.y > pos.y {
				dir.direction = Direction::North;
			}
			
			let mut damage: usize = 0;
			if !game.is_multiplayer {
				// Compute a random number between -3 to 3.
				let mut rng = rand::thread_rng();
				let random_dmg = rng.gen_range(0..7);
				let random_dmg_modifier = random_dmg - 3;
				
				
				// Add random modifier to damage.
				// Damage is (STR / 3) + modifier.
				damage = (str.value / 3) + random_dmg_modifier;
			} else {
				damage = basic_attack_action.damage;
			}
			
			
			// Subtract damage from target HP.
			if damage > hp_current.value {
				hp_current.value = 0;
				
				// Remove Attacker marker component.
				commands.entity(entity).remove::<Attacker>();
			
				// Remove BasicAttack UnitAction.
				unit_actions.unit_actions.remove(0);
				unit_actions.processing_unit_action = false;
				commands.entity(entity).remove::<BasicAttackAction>();
				
				info!("DEBUG: Processed BasicAttack action.");
				
				return;
			} else {
				hp_current.value -= damage;
			}
			
			info!("DEBUG: Unit {:?} did {:?} damage to unit {:?}.", unit_id, damage, target_id);
			info!("DEBUG: Unit {} now has {} HP.", target_id.value, hp_current.value);
			
			// Remove Target marker component from the target.
			commands.entity(target_entity).remove::<Target>();
			
			if game.is_multiplayer {
				// Remove Attacker marker component.
				commands.entity(entity).remove::<Attacker>();
				
				// Remove BasicAttack UnitAction.
				unit_actions.unit_actions.remove(0);
				unit_actions.processing_unit_action = false;
				commands.entity(entity).remove::<BasicAttackAction>();
				
				info!("DEBUG: Processed BasicAttack action.");
				
				return;
			}
			
			match unit_actions.unit_actions[0].0 {
				UnitAction::BasicAttack { target, is_counterattack, damage } => {
					// If it is not already a counter-attack...
					if !is_counterattack {
						// If target is not a ranged unit...
						// Insert a counter-attack.
						match attack_type {
							AttackType::Ranged => { 
								info!("DEBUG: Target is a ranged unit. Won't make a counter-attack.");
							},
							AttackType::Melee => {
								info!("DEBUG: Target is a melee unit. Will make a counter-attack if at range.");
								let target_possible_attacks = find_possible_attacks(map.to_vec(), *target_pos, attack_range.value, *attack_type);
								
								if target_possible_attacks.contains(pos) {
									// Insert a BasicAttack as a counter-attack.
									target_unit_actions.unit_actions.push(UnitActionTuple(UnitAction::BasicAttack {
										target: Pos { x: pos.x, y: pos.y, },
										is_counterattack: true,
										damage: 0,
									}, 0.0));
									
									// Insert the Attacker marker component on the counter-attacking unit.
									commands.entity(target_entity).insert(Attacker {});
									
									// Insert the Target marker component on the unit that did the initial attack.
									// This will result in an infinite Attack and CounterAttack loop for now.
									commands.entity(entity).insert(Target {});
								}
							}
						}
					}
				},
				_ => { empty_system() },
			}
			
		}
		
		// Remove Attacker marker component.
		commands.entity(entity).remove::<Attacker>();
	
		// Remove BasicAttack UnitAction.
		unit_actions.unit_actions.remove(0);
		unit_actions.processing_unit_action = false;
		commands.entity(entity).remove::<BasicAttackAction>();
		
		info!("DEBUG: Processed BasicAttack action.");
	}	
}

//// Prototype
//fn cutscene_1(mut commands: Commands,
//mut swordsman_query: Query<(Entity, &mut UnitActions), With<NakedSwordsman>>,
//) {
//	//info!("DEBUG: Adding third Move UnitAction...");
//	let (entity, mut unit_actions) = swordsman_query.single_mut();
//	unit_actions.unit_actions.push(UnitActionTuple(UnitAction::Move { destination: Pos { x: 2, y: 2, } }, 0.0));
//	unit_actions.unit_actions.push(UnitActionTuple(UnitAction::Move { destination: Pos { x: 4, y: 2, } }, 4.0));
//	unit_actions.unit_actions.push(UnitActionTuple(UnitAction::Move { destination: Pos { x: 6, y: 2, } }, 8.0));
//	info!("DEBUG: Added Move UnitActions.");
//	
//	unit_actions.unit_actions.push(UnitActionTuple(UnitAction::Talk { message: "Lorem ipsum".to_string() }, 12.0));
//	info!("DEBUG: Added Talk UnitAction.");
//}

// Client
fn spawn_units(
mut commands: Commands,
asset_server: Res<AssetServer>,
mut map_query: Query<&mut Map>,
tile_transform_query: Query<&Transform, With<GameText>>,
mut next_state: ResMut<NextState<GameState>>,
) {
	info!("DEBUG: Starting to spawn units...");

	let mut map = &mut map_query.single_mut().map;

	let mut rdr = Reader::from_path("src/the_patrol_ambush_data.csv").unwrap();
	let mut records: Vec<StringRecord> = Vec::new();
	for result in rdr.records(){
		let record = result.unwrap();
		//info!("{:?}", record);
		records.push(record);
	}
	
	for record in records {
		info!("DEBUG: Creating new unit...");
		let entity_id = commands.spawn((
			UnitAttributes {
				unit_id : UnitId { value: record[0].parse().unwrap(), },
				unit_team : UnitTeam { value: record[1].parse().unwrap(), },
				unit_name : UnitName { value: record[2].to_string(), },
				unit_class : UnitClass { value: record[3].to_string(), },
				pos_x : PosX { value: record[4].parse().unwrap(), }, 
				pos_y : PosY { value: record[5].parse().unwrap(), },
				wt_max : WTMax { value: record[6].parse().unwrap(), },
				wt_current : WTCurrent{ value: record[7].parse().unwrap(), },
				hp_max : HPMax { value: record[8].parse().unwrap(), },
				hp_current : HPCurrent { value: record[9].parse().unwrap(), },
				mp_max : MPMax { value: record[10].parse().unwrap(), },
				mp_current : MPCurrent { value: record[11].parse().unwrap(), },
				str : STR { value: record[12].parse().unwrap(), },
				vit : VIT { value: record[13].parse().unwrap(), },
				int : INT { value: record[14].parse().unwrap(), },
				men : MEN { value: record[15].parse().unwrap(), },
				agi : AGI { value: record[16].parse().unwrap(), },
				dex : DEX { value: record[17].parse().unwrap(), },
				luk : LUK { value: record[18].parse().unwrap(), },
				unit_sprite : UnitSprite { value: record[19].to_string(), },
				dir: DIR { direction: Direction::from_string(record[20].to_string()), },
				movement_range: MovementRange { value: record[21].parse().unwrap(), },
				attack_range: AttackRange { value: record[22].parse().unwrap(), },
				attack_type: AttackType::from_string(record[23].to_string()),
			},
			Unit,
			UnitActions { unit_actions: Default::default(), processing_unit_action: false, },
			Pos {
				x: record[4].parse().unwrap(),
				y: record[5].parse().unwrap(),
			},
			MoveActions { move_actions: Vec::new(), },
		)).id();
		
		let mut path_string: String = record[19].to_string();
		path_string.push_str("_east.png");
		
		// Get tile transform.
		let tile_entity = map[record[4].parse::<usize>().unwrap()][record[5].parse::<usize>().unwrap()].3[map[record[4].parse::<usize>().unwrap()][record[5].parse::<usize>().unwrap()].3.len() - 1];
		
		if let Ok(tile_transform) = tile_transform_query.get(tile_entity) {
			let unit_transform = Transform::from_xyz(tile_transform.translation.x, tile_transform.translation.y + 100.0, tile_transform.translation.z + 0.00000001);
			
			commands.entity(entity_id).insert(SpriteBundle {
						texture: asset_server.load(path_string),
						transform: unit_transform,
						..default()
			},);
		}
		
		
		map[record[4].parse::<usize>().unwrap()][record[5].parse::<usize>().unwrap()].2.push(entity_id);
		
		//info!("DEBUG: Unit transform is: {:?}.", Transform::from_xyz((record[4].parse::<usize>().unwrap() as f32) * 256.0 / 2.0 - (record[5].parse::<usize>().unwrap() as f32) * 256.0 / 2.0, (record[4].parse::<usize>().unwrap() as f32) * 128.0 / 2.0 + (record[5].parse::<usize>().unwrap() as f32) * 128.0 / 2.0 + (map[record[4].parse::<usize>().unwrap() as usize][record[5].parse::<usize>().unwrap() as usize].0 as f32) * 15.0 + 100.0, 1.0));
		 
		
		//* (111.0 / (128.0 / 2.0) - 1.0)
	}
	
	info!("DEBUG: Finished spawning units.");
	info!("DEBUG: Setting GameState to Ambush.");
	next_state.set(GameState::Ambush);
	info!("DEBUG: Set GameState to Ambush.");
}

// Client
fn loading_complete(client: Res<Client>, mut next_state: ResMut<NextState<GameState>>, state: Res<State<GameState>>) {
	info!("DEBUG: Sending LoadingComplete message...");
	client
		.connection()
		.try_send_message(ClientMessage::LoadingComplete);
	info!("DEBUG: Sent LoadingComplete message.");
	
	info!("DEBUG: Setting GameState to Wait...");
	//commands.insert_resource(NextState(GameState::Wait));
	next_state.set(GameState::Wait);	
	info!("DEBUG: Set GameState to Wait.");
	info!("DEBUG: Current state is {:?}.", state.get());
}

fn current_state(state: Res<State<GameState>>) {
	info!("DEBUG: Current state is: {:?}.", state.get());
}

fn current_state2(state: Res<State<GameState>>) {
	info!("DEBUG: Current state is: {:?}.", state.get());
}

fn change_state(mut next_state: ResMut<NextState<GameState>>, mut input: ResMut<Input<KeyCode>>) {

	if input.just_pressed(KeyCode::Space) {

		next_state.set(GameState::Wait);
	}
}

// Test
fn get_toggle_console_key(console_config: Res<ConsoleConfiguration>) {
	for key in &console_config.keys {
		
		match key {
			ToggleConsoleKey::KeyCode(key_code) => {
				info!("Console toggle key is: {:?}.", key_code);
			},
			_ => { empty_system(); },
		}
	}
}

// Test
fn do_nothing_command(mut log: ConsoleCommand<DoNothingCommand>, mut unit_actions_query: Query<&mut UnitActions>) {
    if let Some(Ok(do_nothing_command)) = log.take() {
        // handle command
        for mut unit_actions in unit_actions_query.iter_mut() {
			unit_actions.unit_actions.push(UnitActionTuple::default());
			info!("DEBUG: Added DoNothing UnitAction.");
        }
    }
}

// Prototype
fn talk_command(mut log: ConsoleCommand<TalkCommand>, mut unit_query: Query<(Entity, &UnitId, &mut UnitActions)>) {
	if let Some(Ok(TalkCommand { unit_id, msg })) = log.take() {
        // handle command
        // Find unit with ID = unit_id.
        for (entity_id, unit_id2, mut unit_actions) in unit_query.iter_mut() {
			if unit_id2.value == unit_id {
				unit_actions.unit_actions.push(UnitActionTuple(UnitAction::Talk { message: msg.clone() }, 0.0));
			}
        }
    }
}

// Prototype
fn move_command(mut log: ConsoleCommand<MoveCommand>, mut unit_query: Query<(Entity, &UnitId, &mut UnitActions, &Pos)>) {
	if let Some(Ok(MoveCommand { unit_id, x, y })) = log.take() {
        // handle command
        // Find unit with ID = unit_id.
        for (entity_id, unit_id2, mut unit_actions, pos) in unit_query.iter_mut() {
			if unit_id2.value == unit_id {
				unit_actions.unit_actions.push(UnitActionTuple(UnitAction::Move {
					origin: Pos { x: pos.x, y: pos.y, },
					destination: Pos { x: x, y: y },
					timer: Timer::from_seconds(4.0, TimerMode::Once),
				}, 0.0));
			}
        }
    }
}

// Prototype
fn ars_militaris_demo(
mut map_query: Query<&mut Map>,
mut units_query: Query<(Entity, &UnitId, &mut UnitActions, &Pos)>,
mut target_units_query: Query<(Entity, &Pos), With<Unit>>,
mut demo_data: ResMut<DemoData>,
time: Res<Time>,
) {
	let map = &map_query.single().map;
	
//	let mut rng = rand::thread_rng();
//	// Compute a random unit.
//	let random_unit_id = rng.gen_range(0..units_query.iter().len());
//	
//	// Find a random unit.
//	for (entity, unit_id, mut unit_actions, pos) in units_query.iter_mut() {
//		if (unit_id.value - 1) == random_unit_id {
//			// Find another unit.
//			for (target_entity, target_pos) in target_units_query.iter_mut() {
//				if target_entity != entity {
//					// Compute the new target position.
//					let target_pos = Pos { x: pos.x + 1, y: pos.y, };
//					
//					// Insert a random Move UnitAction on the first unit.
//					let mut rng2 = rand::thread_rng();
//					let random_usize: usize = rng2.gen_range(0..map.len());
//					
//					let mut rng3 = rand::thread_rng();
//					let random_usize2: usize = rng3.gen_range(0..map[0].len());
//					
//					// Compute a move of a single tile in a random direction
//										
//					unit_actions.unit_actions.push(UnitActionTuple(UnitAction::Move {
//						origin: Pos { x: pos.x, y: pos.y, },
//						destination: Pos { x: target_pos.x, y: target_pos.y, },
//						timer: Timer::from_seconds(4.0, TimerMode::Once),
//					}, (time.elapsed().as_secs_f32()) + 6.0));
//					
//	//				// Insert a Move UnitAction on the first unit to move towards the target unit.
//	//				unit_actions.unit_actions.push(UnitActionTuple(UnitAction::Move { destination: target_pos, }, 0.0));
//					break;
//				} else {
//					continue;
//				}			
//			}
//		} else {
//			continue;
//		}
//		break;
//	}
	
//	// Find a unit with UnitId 1.
//	for (entity, unit_id, mut unit_actions, pos) in units_query.iter_mut() {
//		if unit_id.value == 1 {
//			// Move 1 tile forward.
//			unit_actions.unit_actions.push(UnitActionTuple(UnitAction::Move {
//						origin: Pos { x: pos.x, y: pos.y, },
//						destination: Pos { x: pos.x + 1, y: pos.y, },
//						timer: Timer::from_seconds(4.0, TimerMode::Once),
//					}, (time.elapsed().as_secs_f32()) + 6.0));
//			
//		} else {
//			continue;
//		}
//		break;
//	}

	// Get the current unit and make a `Move` `UnitAction` towards its target.
	for (entity, unit_id, mut unit_actions, pos) in units_query.iter_mut() {
		if unit_id.value == demo_data.current_unit.value {
			unit_actions.unit_actions.push(UnitActionTuple(UnitAction::Move {
				origin: Pos { x: pos.x, y: pos.y, },
				destination: Pos { x: pos.x + 1, y: pos.y, },
				timer: Timer::from_seconds(4.0, TimerMode::Once),
			}, (time.elapsed().as_secs_f32()) + 6.0));
			
			// Set the next current unit.
			if demo_data.current_unit.value == units_query.iter_mut().len() {
				demo_data.current_unit.value = 1;
			} else {
				demo_data.current_unit.value += 1;
			}
		} else {
			continue;
		}
		break;
	}
}

// Prototype
fn single_player_pause(
mut input: ResMut<Input<KeyCode>>,
mut next_state: ResMut<NextState<GameState>>,
) {
	if input.just_pressed(KeyCode::F3) {
		info!("DEBUG: Pausing the game...");
		info!("DEBUG: Setting GameState to SinglePlayerPause...");
		next_state.set(GameState::SinglePlayerPause);
		info!("DEBUG: Set GameState to SinglePlayerPause.");
	}
}

// Prototype
fn handle_single_player_pause_state(
mut input: ResMut<Input<KeyCode>>,
mut next_state: ResMut<NextState<GameState>>,
) {
	if input.just_pressed(KeyCode::F3) {
		info!("DEBUG: Unpausing the game...");
		info!("DEBUG: Setting GameState to Ambush...");
		next_state.set(GameState::Ambush);
		info!("DEBUG: Set GameState to Ambush.");
	}
}

// Prototype
fn first_ai(
mut map_query: Query<&mut Map>,
mut unit_query: Query<(Entity, &UnitId, &mut UnitActions, &Pos, &mut WTCurrent, &WTMax), With<CurrentUnit>>,
time: Res<Time>,
mut commands: Commands,
mut next_state: ResMut<NextState<TurnState>>,
) {
	let map = &map_query.single().map;
	
	// Get current unit.
	if let (entity, unit_id, mut unit_actions, pos, mut wt_current, wt_max) = unit_query.single_mut() {
		// Insert `Move` `UnitAction`.
		unit_actions.unit_actions.push(UnitActionTuple(UnitAction::Move {
				origin: Pos { x: pos.x, y: pos.y, },
				destination: Pos { x: pos.x - 1, y: pos.y, },
				timer: Timer::from_seconds(4.0, TimerMode::Once),
			}, 0.0));
		
		// End turn.
		wt_current.value = wt_max.value;
		
		commands.entity(entity).remove::<CurrentUnit>();
		
		info!("DEBUG: AI has finished its turn.");
		info!("DEBUG: Setting TurnState to Wait...");
		next_state.set(TurnState::Wait);
		info!("DEBUG: Set TurnState to Wait.");
	}
	
	
	
}

// Prototype
fn handle_unit_directions(
mut units_query: Query<(Entity, &mut Handle<Image>, &DIR, &UnitSprite)>,
asset_server: Res<AssetServer>,
) {
	for (entity, mut sprite, dir, unit_sprite) in units_query.iter_mut() {
		match dir.direction {
			Direction::East => {
				let mut sprite_path = unit_sprite.value.clone();
				sprite_path.push_str("_east.png");
				*sprite = asset_server.load(&*sprite_path);
			},
			Direction::South => {
				let mut sprite_path = &mut unit_sprite.value.clone();
				sprite_path.push_str("_south.png");
				*sprite = asset_server.load(&*sprite_path);
			},
			Direction::West => {
				let mut sprite_path = &mut unit_sprite.value.clone();
				sprite_path.push_str("_west.png");
				*sprite = asset_server.load(&*sprite_path);
			},
			Direction::North => {
				let mut sprite_path = &mut unit_sprite.value.clone();
				sprite_path.push_str("_north.png");
				*sprite = asset_server.load(&*sprite_path);
			},
		}
	}
}

// Prototype
fn choose_move(
mut commands: Commands,
map_query: Query<&Map>,
unit_query: Query<(Entity, &Pos, &MovementRange), With<CurrentUnit>>,
tile_query: Query<&Transform, With<GameText>>,
asset_server: Res<AssetServer>,
) {
	let map = &map_query.single().map;
	let (entity, pos, movement_range) = unit_query.single();
	
	let possible_movements = find_possible_movements(map.to_vec(), *pos, movement_range.value);
	info!("DEBUG: Possible movements are: {:?}.", possible_movements);
	
	// Spawn the MoveTile indicators.
	
	
	for tile in &possible_movements {
	
		// Compute the indicator position, based on the tile.
		if let Ok(tile_transform) = tile_query.get(map[tile.x][tile.y].3[map[tile.x][tile.y].3.len() - 1]) {
			
			commands.spawn((SpriteBundle {
				sprite: Sprite {
					color: Color::rgba(0.0, 0.0, 1.0, 0.5),
					..default()
				},
				texture: asset_server.load("move_tile.png"),
				transform: Transform::from_xyz(tile_transform.translation.x, tile_transform.translation.y, tile_transform.translation.z + 0.000000025),
				..default()
			},
			MoveTile {},
			));
		}
		
		
	}
	
	commands.entity(entity).insert(MoveTiles { move_tiles: possible_movements, });
}

// Prototype
fn start_choose_move(
mut input: ResMut<Input<KeyCode>>,
mut next_state: ResMut<NextState<TurnState>>,
) {
	if input.just_pressed(KeyCode::M) {
		info!("DEBUG: Setting TurnState to ChooseMove...");
		next_state.set(TurnState::ChooseMove);
		info!("DEBUG: Set TurnState to ChooseMove.");
	}
}

fn handle_choose_move(
mut commands: Commands,
mut input: ResMut<Input<KeyCode>>,
mut unit_query: Query<(Entity, &MoveTiles, &Pos, &mut UnitActions), With<CurrentUnit>>,
cursor_query: Query<&Cursor>,
move_tiles_query: Query<Entity, With<MoveTile>>,
mut next_state: ResMut<NextState<TurnState>>,
game: Res<Game>,
mut client: ResMut<Client>,
) {
	let cursor = cursor_query.single();
	let (entity, move_tiles, pos, mut unit_actions) = unit_query.single_mut();

	if input.just_pressed(KeyCode::M) {
		// Check if cursor is in a MoveTile.
		let cursor_pos = Pos { x: cursor.x, y: cursor.y, };
		if move_tiles.move_tiles.contains(&cursor_pos) {
			// Remove the MoveTiles
			for entity in move_tiles_query.iter() {
				commands.entity(entity).despawn();
			}
			
			if !game.is_multiplayer {
				// Game is in single-player mode.
				// Insert a `Move` `UnitAction` towards the target tile.
				info!("DEBUG: Unit can move to this tile.");
				info!("DEBUG: Moving unit...");
				
				unit_actions.unit_actions.push(UnitActionTuple(UnitAction::Move {
						origin: Pos { x: pos.x, y: pos.y, },
						destination: Pos { x: cursor.x, y: cursor.y },
						timer: Timer::from_seconds(4.0, TimerMode::Once),
				}, 0.0));
				
				// Set State
				next_state.set(TurnState::Turn);
			} else {
				// Game is in multiplayer mode.
				// Send a `ClientMessage::Move` message to the server.
				info!("DEBUG: Unit can move to this tile.");
				info!("DEBUG: Sending `Move` message...");
				client
					.connection()
					.try_send_message(ClientMessage::Move { 
						origin: Pos { x: pos.x, y: pos.y, },
						destination: Pos { x: cursor.x, y: cursor.y },
					});
				info!("DEBUG: Sent Move message.");
				
			}			
		}
	}
	
	if input.just_pressed(KeyCode::Escape) {
		// Remove the MoveTiles
		for entity in move_tiles_query.iter() {
			commands.entity(entity).despawn();
		}
	
		info!("Setting TurnState back to Turn...");
		next_state.set(TurnState::Turn);
		info!("Set TurnState back to Turn.");
	}
}

// Prototype
fn start_choose_attack(
mut input: ResMut<Input<KeyCode>>,
mut next_state: ResMut<NextState<TurnState>>,
) {
	if input.just_pressed(KeyCode::F) {
		info!("DEBUG: Setting TurnState to ChooseAttack...");
		next_state.set(TurnState::ChooseAttack);
		info!("DEBUG: Set TurnState to ChooseAttack.");
	}
}

fn choose_attack(mut commands: Commands,
map_query: Query<&Map>,
unit_query: Query<(Entity, &Pos, &AttackRange, &AttackType), With<CurrentUnit>>,
tile_query: Query<&Transform, With<GameText>>,
asset_server: Res<AssetServer>,
) {
	let map = &map_query.single().map;
	let (entity, pos, attack_range, attack_type) = unit_query.single();
	
	let possible_attacks = find_possible_attacks(map.to_vec(), *pos, attack_range.value, attack_type.clone());
	info!("DEBUG: Possible attacks are: {:?}.", possible_attacks);
	
	// Spawn the AttackTile indicators.
	
	
	for tile in &possible_attacks {
	
		// Compute the indicator position, based on the tile.
		if let Ok(tile_transform) = tile_query.get(map[tile.x][tile.y].3[map[tile.x][tile.y].3.len() - 1]) {
			
			commands.spawn((SpriteBundle {
				sprite: Sprite {
					color: Color::rgba(1.0, 0.0, 0.0, 0.5),
					..default()
				},
				texture: asset_server.load("attack_tile.png"),
				transform: Transform::from_xyz(tile_transform.translation.x, tile_transform.translation.y, tile_transform.translation.z + 0.000000025),
				..default()
			},
			AttackTile {},
			));
		}
		
		
	}
	
	commands.entity(entity).insert(AttackTiles { attack_tiles: possible_attacks, });
}

// Prototype
fn handle_choose_attack(
mut commands: Commands,
map_query: Query<&Map>,
mut input: ResMut<Input<KeyCode>>,
mut unit_query: Query<(Entity, &AttackTiles, &Pos, &mut UnitActions), With<CurrentUnit>>,
cursor_query: Query<&Cursor>,
attack_tiles_query: Query<Entity, With<AttackTile>>,
client: Res<Client>,
game: Res<Game>,
mut next_state: ResMut<NextState<TurnState>>,
) {
	let map = &map_query.single().map;

	let cursor = cursor_query.single();
	let (entity, attack_tiles, pos, mut unit_actions) = unit_query.single_mut();

	if input.just_pressed(KeyCode::F) {
		// Check if cursor is in a AttackTile.
		let cursor_pos = Pos { x: cursor.x, y: cursor.y, };
		if attack_tiles.attack_tiles.contains(&cursor_pos) {
			
			if !game.is_multiplayer {
				// Game is in single-player mode.
			
				// Check it there is an unit on the tile.
				// So that the player won't attack empty tiles.
				// In the future we will add the option to attack empty tiles.
				// To be in accordance with Tactics Ogre.
				
				if map[cursor.x][cursor.y].2.len() > 0 {
					// Remove the AttackTiles
					for entity in attack_tiles_query.iter() {
						commands.entity(entity).despawn();
					}
					
					// Insert an `Attack` `UnitAction` towards the target unit.
					info!("DEBUG: Unit can attack this tile.");
					info!("DEBUG: Attacking...");
					unit_actions.unit_actions.push(UnitActionTuple(UnitAction::BasicAttack {
						target: Pos { x: cursor.x, y: cursor.y, },
						is_counterattack: false,
						damage: 0,
					}, 0.0));
					
					// Insert an `Attacker` marker component on the attacking unit.
					commands.entity(entity).insert(Attacker {});
					
					// Insert the `Target` marker component on the target unit.
					let target_entity = map[cursor.x][cursor.y].2[0];
					commands.entity(target_entity).insert(Target {});
									
					// Remove the AttackTiles component from the unit.
					commands.entity(entity).remove::<AttackTiles>();
					
					// Set State
					next_state.set(TurnState::Turn);
				}
			} else {
				// Game is in multiplayer mode.
				info!("DEBUG: Sending BasicAttack message...");
				client
					.connection()
					.try_send_message(ClientMessage::BasicAttack {
						attacker: Pos { x: pos.x, y: pos.y, },
						target: Pos { x: cursor.x, y: cursor.y, },
						damage: 0,
					});
				info!("DEBUG: Sent BasicAttack message.");
				
				// Remove the AttackTiles
				for entity in attack_tiles_query.iter() {
					commands.entity(entity).despawn();
				}
			}
		}
	}
	
	if input.just_pressed(KeyCode::Escape) {
		// Remove the AttackTile indicators
		for entity in attack_tiles_query.iter() {
			commands.entity(entity).despawn();
		}
		
		// Remove the AttackTiles component from the unit.
		commands.entity(entity).remove::<AttackTiles>();
		
		info!("Setting TurnState back to Turn...");
		next_state.set(TurnState::Turn);
		info!("Set TurnState back to Turn.");
	}
}

// Prototype
fn handle_unit_death(
mut commands: Commands,
mut map_query: Query<&mut Map>,
unit_query: Query<(Entity, &UnitId, &Pos, &HPCurrent)>,
game: Res<Game>,
mut next_state: ResMut<NextState<TurnState>>,
) {
	for (entity, unit_id, pos, hp_current) in unit_query.iter() {
		if hp_current.value == 0 {
			// Remove unit.
			let mut map = &mut map_query.single_mut().map;
			map[pos.x][pos.y].2.remove(0);
			
			info!("DEBUG: Unit {} has died. Removing it...", unit_id.value);
			commands.entity(entity).despawn();
			
			// If that unit has the current turn, end it.
			if unit_id.value == game.current_unit {
				// Set TurnState to Wait.
				info!("DEBUG: Setting TurnState to Wait...");
				next_state.set(TurnState::Wait);
				info!("DEBUG: Set TurnState to Wait.");
			}
		}
	}
}

// Prototype
fn handle_ambush_game_over(
unit_query: Query<(Entity, &UnitTeam)>,
mut next_state: ResMut<NextState<GameState>>,
mut next_turn_state: ResMut<NextState<TurnState>>,
mut game: ResMut<Game>,
) {
	let mut player_still_alive: bool = false;
	let mut ai_still_alive: bool = false;
	for (entity, unit_team) in unit_query.iter() {
		if unit_team.value == 1 {
			player_still_alive = true;
		}
		
		if unit_team.value == 2 {
			ai_still_alive = true;
		}
	}
	
	if !player_still_alive {
		// Player lost.
		info!("DEBUG: Game over. Winner is AI.");
		game.winner = ControlledBy::AI;
		
		info!("DEBUG: Setting GameState to MainMenu...");
		next_state.set(GameState::MainMenu);
		info!("DEBUG: Set GameState to MainMenu.");
		info!("DEBUG: Setting TurnState to Wait...");
		next_turn_state.set(TurnState::Wait);
		info!("DEBUG: Set TurnState to Wait.");
	}
	
	if !ai_still_alive {
		// Player won.
		info!("DEBUG: Game over. Winner is Player.");
		game.winner = ControlledBy::Player;
		
		info!("DEBUG: Setting GameState to MainMenu...");
		next_state.set(GameState::MainMenu);
		info!("DEBUG: Set GameState to MainMenu.");
		info!("DEBUG: Setting TurnState to Wait...");
		next_turn_state.set(TurnState::Wait);
		info!("DEBUG: Set TurnState to Wait.");
	}
}

// Prototype
fn handle_ambush_to_main_menu_transition(
mut commands: Commands,
mut query: Query<Entity, Without<Window>>,
) {
	for entity in query.iter() {
		commands.entity(entity).despawn();
	}
}

// Prototype
fn setup_main_menu(
mut commands: Commands,
asset_server: Res<AssetServer>,
) {
	commands.spawn(Camera2dBundle::default());

//	// Spawn Ars Militaris Logo.
//	commands.spawn(ImageBundle {
//		style: Style {
//			width: Val::Percent(100.0),
//			height: Val::Percent(70.0),
//			bottom: Val::Percent(30.0),
//			..default()
//		}, 
//        image: asset_server.load("arsmilitaris_logo.png").into(),
//        ..default()
//    });

	 commands
        .spawn((NodeBundle {
            style: Style {
                width: Val::Percent(100.0),
                ..default()
            },
            ..default()
        },
        MainMenuUI {},
        ))
        .with_children(|parent| {
			parent
                .spawn(ImageBundle {
					style: Style {
						width: Val::Percent(100.0),
						height: Val::Percent(100.0),
						bottom: Val::Percent(0.0),
						..default()
					}, 
					image: asset_server.load("arsmilitaris_logo.png").into(),
					..default()
                })
                .with_children(|parent| {
					parent
						.spawn((ButtonBundle {
							style: Style {
								width: Val::Percent(20.0),
								height: Val::Percent(10.0),
								border: UiRect::all(Val::Px(5.0)),
								//bottom: Val::Percent(10.0),
								// horizontally center child text
								justify_content: JustifyContent::Center,
								// vertically center child text
								align_items: AlignItems::Center,
								..default()
							},
							border_color: BorderColor(Color::BLACK),
							background_color: BackgroundColor(Color::BLACK),
							..default()
						},
						StartDemoButton {},
						))
						.with_children(|parent| {
							parent.spawn(TextBundle::from_section(
								"Demo",
								TextStyle {
									font: asset_server.load("fonts/FiraSans-Bold.ttf"),
									font_size: 40.0,
									color: Color::rgb(0.9, 0.9, 0.9),
								},
							));
						});
					parent
						.spawn((ButtonBundle {
							style: Style {
								width: Val::Percent(20.0),
								height: Val::Percent(10.0),
								border: UiRect::all(Val::Px(5.0)),
								//bottom: Val::Percent(10.0),
								// horizontally center child text
								justify_content: JustifyContent::Center,
								// vertically center child text
								align_items: AlignItems::Center,
								..default()
							},
							border_color: BorderColor(Color::BLACK),
							background_color: BackgroundColor(Color::BLACK),
							..default()
						},
						StartAmbushButton {},
						))
						.with_children(|parent| {
							parent.spawn(TextBundle::from_section(
								"Ambush",
								TextStyle {
									font: asset_server.load("fonts/FiraSans-Bold.ttf"),
									font_size: 40.0,
									color: Color::rgb(0.9, 0.9, 0.9),
								},
							));
						});
					parent
						.spawn((ButtonBundle {
							style: Style {
								width: Val::Percent(20.0),
								height: Val::Percent(10.0),
								border: UiRect::all(Val::Px(5.0)),
								//bottom: Val::Percent(10.0),
								// horizontally center child text
								justify_content: JustifyContent::Center,
								// vertically center child text
								align_items: AlignItems::Center,
								..default()
							},
							border_color: BorderColor(Color::BLACK),
							background_color: BackgroundColor(Color::BLACK),
							..default()
						},
						StartMultiplayerButton {},
						))
						.with_children(|parent| {
							parent.spawn(TextBundle::from_section(
								"Multiplayer",
								TextStyle {
									font: asset_server.load("fonts/FiraSans-Bold.ttf"),
									font_size: 40.0,
									color: Color::rgb(0.9, 0.9, 0.9),
								},
							));
						});
					parent
						.spawn((ButtonBundle {
							style: Style {
								width: Val::Percent(20.0),
								height: Val::Percent(10.0),
								border: UiRect::all(Val::Px(5.0)),
								//bottom: Val::Percent(10.0),
								// horizontally center child text
								justify_content: JustifyContent::Center,
								// vertically center child text
								align_items: AlignItems::Center,
								..default()
							},
							border_color: BorderColor(Color::BLACK),
							background_color: BackgroundColor(Color::BLACK),
							..default()
						},
						QuitGameButton {},
						))
						.with_children(|parent| {
							parent.spawn(TextBundle::from_section(
								"Quit",
								TextStyle {
									font: asset_server.load("fonts/FiraSans-Bold.ttf"),
									font_size: 40.0,
									color: Color::rgb(0.9, 0.9, 0.9),
								},
							));
						});
					});
       });
}

// Prototype
fn tear_down_main_menu(
mut commands: Commands,
main_menu_query: Query<Entity, With<MainMenuUI>>,
main_menu_camera_query: Query<Entity, With<Camera>>,
) {
	for entity in main_menu_query.iter() {
		commands.entity(entity).despawn();
	}
	
	for camera_entity in main_menu_camera_query.iter() {
		commands.entity(camera_entity).despawn();
	}
}

// Prototype
fn handle_main_menu_buttons(
mut commands: Commands,
mut ambush_button_query: Query<(&Interaction), (Changed<Interaction>, With<Button>, With<StartAmbushButton>)>,
mut multiplayer_button_query: Query<(&Interaction), (Changed<Interaction>, With<Button>, With<StartMultiplayerButton>)>,
mut quit_button_query: Query<(&Interaction), (Changed<Interaction>, With<Button>, With<QuitGameButton>)>,
query: Query<Entity>,
client: Res<Client>,
mut next_state: ResMut<NextState<GameState>>,

) {
	for interaction in ambush_button_query.iter() {
		match *interaction {
			Interaction::Pressed => {
				next_state.set(GameState::LoadAmbush);
			},
			_ => { empty_system(); },
		}
	}
	
	for interaction in multiplayer_button_query.iter() {
		match *interaction {
			Interaction::Pressed => {
				client
					.connection()
					.try_send_message(ClientMessage::StartGame);
			},
			_ => { empty_system(); },
		}
	}
	
	for interaction in quit_button_query.iter() {
		match *interaction {
			Interaction::Pressed => {
				for entity in query.iter() {
					commands.entity(entity).despawn();
				}
			},
			_ => { empty_system(); },
		}
	}
}

// Prototype
fn set_loading_complete(
mut next_state: ResMut<NextState<GameState>>) {
	info!("DEBUG: Setting GameState to LoadingComplete...");
	next_state.set(GameState::LoadingComplete);	
	info!("DEBUG: Set GameState to LoadingComplete.");
}

// Prototype
fn is_multiplayer(
game: Res<Game>,
) -> bool {
	return game.is_multiplayer;
}

// Prototype
fn is_singleplayer(
game: Res<Game>,
) -> bool {
	return !game.is_multiplayer;
}

// Utility
fn find_path(map: Vec<Vec<(usize, TileType, Vec<Entity>, Vec<Entity>)>>, start: Pos, destination: Pos) -> Option<Vec<Pos>> {
    // Define a heuristic function that estimates the distance between two positions.
    // In this case, we use the Manhattan distance (taxicab distance).
    let heuristic = |pos: &Pos| -> usize {
        (pos.x as isize - destination.x as isize).abs() as usize
            + (pos.y as isize - destination.y as isize).abs() as usize
    };

	// Use RefCell to wrap the map so that it can be mutated inside the closure.
    let map_cell = RefCell::new(map);

    // Define a function that returns the valid neighboring positions of a given position.
    let neighbors = |pos: &Pos| -> Vec<(Pos, usize)> {
		// Access the map using borrow_mut() to allow mutation.
		let mut map = map_cell.borrow_mut();
    
        // Add logic to get the valid neighboring positions based on your map layout.
        // For example, avoid diagonal moves and ensure the position is within the map bounds.
        // For simplicity, let's assume you have a function called `get_valid_neighbors`.
        get_valid_neighbors((&mut map).to_vec(), *pos)
    };

    // Use the `astar` function from the pathfinding library to find the path.
    let result = astar(&start, neighbors, heuristic, |&pos| pos == destination);

    if let Some((path, _)) = result {
        Some(path)
    } else {
        None
    }
}

// Utility
fn get_valid_neighbors(map: Vec<Vec<(usize, TileType, Vec<Entity>, Vec<Entity>)>>, pos: Pos) -> Vec<(Pos, usize)> {
	let mut neighbors: Vec<(Pos, usize)> = Vec::new(); 
	
	// Check if tile is at North edge.
	if pos.y == map[0].len() - 1 {
		// Tile is at North edge. Don't add North neighbor.
	} else {
		// Tile is not at North edge. 
		// Check if there's a unit on the tile.
		// If there isn't, add North neighbor.
		if map[pos.x][pos.y + 1].2.len() == 0 {
			neighbors.push((Pos { x: pos.x, y: pos.y + 1, }, 0));
		}
	}
	
	// Check if tile is at South edge.
	if pos.y == 0 {
		// Tile is at South edge. Don't add South neighbor.
	} else {
		// Tile is not at South edge.
		// Check if there's a unit on the tile.
		// If there isn't, add South neighbor.
		if map[pos.x][pos.y - 1].2.len() == 0 {
			neighbors.push((Pos { x: pos.x, y: pos.y - 1, }, 0));
		}
	}
	
	// Check if tile is at East edge.
	if pos.x == map.len() - 1 {
		// Tile is at East edge. Don't add East neighbor.
	} else {
		// Tile is not at East edge.
		// Check if there's a unit on the tile.
		// If there isn't, add East neighbor.
		if map[pos.x + 1][pos.y].2.len() == 0 {
			neighbors.push((Pos { x: pos.x + 1, y: pos.y, }, 0));
		}
	} 
	
	// Check if tile is at West edge.
	if pos.x == 0 {
		// Tile is at West edge. Don't add West neighbor.
	} else {
		// Tile is not at West edge.
		// Check if there's a unit on the tile.
		// If there isn't, add West neighbor.
		if map[pos.x - 1][pos.y].2.len() == 0 {
			neighbors.push((Pos { x: pos.x - 1, y: pos.y, }, 0));
		}
	}
	
	return neighbors;
}

use std::collections::HashSet;

// Prototype
fn find_possible_movements(map: Vec<Vec<(usize, TileType, Vec<Entity>, Vec<Entity>)>>, start: Pos, mut movement_range: isize) -> Vec<Pos> {
    let mut possible_tiles_vec = Vec::new();
    movement_range -= 1;
	
	let mut visited_tiles = HashSet::new();
	
	if movement_range >= 0 {
		// Get neighbors.
		let neighbors = get_valid_neighbors(map.clone(), start);
		for neighbor in &neighbors {
			if visited_tiles.insert(neighbor.0) {
				possible_tiles_vec.push(neighbor.0);
				let mut recursive_possible_tiles = find_possible_movements(map.clone(), neighbor.0, movement_range);
				
				for possible_tile in recursive_possible_tiles {
					if !possible_tiles_vec.contains(&possible_tile) {
						possible_tiles_vec.push(possible_tile);
					}
				}
			}
		}
    }

    possible_tiles_vec
}

// Utility
fn get_valid_attack_neighbors(map: Vec<Vec<(usize, TileType, Vec<Entity>, Vec<Entity>)>>, pos: Pos) -> Vec<(Pos, usize)> {
	let mut neighbors: Vec<(Pos, usize)> = Vec::new(); 
	
	// Check if tile is at North edge.
	if pos.y == map[0].len() - 1 {
		// Tile is at North edge. Don't add North neighbor.
	} else {
		// Tile is not at North edge. 
		neighbors.push((Pos { x: pos.x, y: pos.y + 1, }, 0));
	}
	
	// Check if tile is at South edge.
	if pos.y == 0 {
		// Tile is at South edge. Don't add South neighbor.
	} else {
		// Tile is not at South edge.
		neighbors.push((Pos { x: pos.x, y: pos.y - 1, }, 0));
	}
	
	// Check if tile is at East edge.
	if pos.x == map.len() - 1 {
		// Tile is at East edge. Don't add East neighbor.
	} else {
		// Tile is not at East edge.
		neighbors.push((Pos { x: pos.x + 1, y: pos.y, }, 0));
	} 
	
	// Check if tile is at West edge.
	if pos.x == 0 {
		// Tile is at West edge. Don't add West neighbor.
	} else {
		// Tile is not at West edge.
		neighbors.push((Pos { x: pos.x - 1, y: pos.y, }, 0));
	}
	
	return neighbors;
}

// Prototype
fn find_possible_attacks(map: Vec<Vec<(usize, TileType, Vec<Entity>, Vec<Entity>)>>, start: Pos, mut attack_range: isize, attack_type: AttackType) -> Vec<Pos> {
    let mut possible_tiles_vec = Vec::new();
    
    match attack_type {
		AttackType::Melee => {
			// Get neighbors.
			let neighbors = get_valid_attack_neighbors(map.clone(), start);
			for neighbor in &neighbors {
				possible_tiles_vec.push(neighbor.0);
			}
			
			for neighbor in &neighbors {
				for i in 1..attack_range {
					// If there is an unit on the tile, don't search for more neighbors.
					if map[neighbor.0.x][neighbor.0.y].2.len() > 0 {
						break;
					}
					// Else...
					// Add next neighbor.
					let neighbors = get_valid_attack_neighbors(map.clone(), neighbor.0);
					for neighbor in neighbors {
						if (neighbor.0.x == start.x || neighbor.0.y == start.y) {
							if !(neighbor.0.x == start.x && neighbor.0.y == start.y) {
								if !(possible_tiles_vec.contains(&neighbor.0)) {
									possible_tiles_vec.push(neighbor.0);
								}
							}
						}
					}
				}			
			}
		},
		AttackType::Ranged => {
			attack_range -= 1;
			
			let mut visited_tiles = HashSet::new();
			
			if attack_range >= 0 {
				// Get neighbors.
				let neighbors = get_valid_attack_neighbors(map.clone(), start);
				for neighbor in &neighbors {
					if visited_tiles.insert(neighbor.0) {
						possible_tiles_vec.push(neighbor.0);
						let mut recursive_possible_tiles = find_possible_attacks(map.clone(), neighbor.0, attack_range, attack_type);
						
						for possible_tile in recursive_possible_tiles {
							if !possible_tiles_vec.contains(&possible_tile) {
								possible_tiles_vec.push(possible_tile);
							}
						}
					}
				}
			}
		},
		_ => {
			empty_system();
		},
    }
    
    

    possible_tiles_vec
}

// Logging
fn custom_panic_hook(info: &std::panic::PanicInfo) {
    // Perform any necessary logging or error handling here
    error!("Panic occurred: {:?}", info);
}

//// Logging
//fn setup_logging(world: World) -> Logger {
//    let file = File::create("amclient.slog").expect("Failed to create log file");
//
//    let decorator = TermDecorator::new().build();
//    let stdout_drain = Mutex::new(slog_term::FullFormat::new(decorator).build()).fuse();
//
//    let file_drain = Mutex::new(slog_term::FullFormat::new(slog_term::PlainDecorator::new(file))
//        .use_original_order()
//        .build())
//        .fuse();
//	
//	let console_drain = Mutex::new(BevyConsoleDrain { world: world, }).fuse();
//	
//    // Combine the stdout and file drains.
//    let stdout_and_file_drain = slog::Duplicate::new(stdout_drain, file_drain).fuse();
//	
//	// Combine the `stdout_and_file_drain` with the `console_drain`.
//	let log_drain = slog::Duplicate::new(stdout_and_file_drain, console_drain).fuse();
//	
//    let logger = slog::Logger::root(log_drain, o!());
//
//    logger
//}
//
//// Logging
//fn test_slog(slog: Res<Slog>) {
//	slog::info!(slog.logger, "This is a test for slog.");
//}
//
//// Logging
//struct BevyConsoleDrain {
//	world: World,
//}
//
//impl Drain for BevyConsoleDrain {
//    type Ok = ();
//    type Err = Box<dyn Error + Send + Sync>;
//
//    fn log(&self, record: &Record, values: &OwnedKVList) -> Result<Self::Ok, Self::Err> {
//        // Create the log message using the record and values
//        let msg = format!("{}", record.msg());
//        
//        if let Ok(mut events) = self.world.get_resource_mut::<Event<PrintConsoleLine>>() {
//            events.send(PrintConsoleLine(msg.clone()));
//        }
//
//        Ok(())
//    }
//}

fn toggle_console(mut console_open: ResMut<ConsoleOpen>) {
	console_open.open = true;
}

fn test_write_to_console(mut console_line: EventWriter<PrintConsoleLine>) {
	console_line.send(PrintConsoleLine::new("DEBUG: This is a test in writing to the console.".into()));
}

fn empty_system() {

}