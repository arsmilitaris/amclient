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

use csv::Reader;
use csv::StringRecord;

use bevy_quinnet::{
    client::{
        certificate::CertificateVerificationMode, Client, connection::ConnectionConfiguration, connection::ConnectionEvent,
        QuinnetClientPlugin, 
    },
    shared::ClientId,
};

use bevy_console::{ConsoleConfiguration, ConsolePlugin, ToggleConsoleKey};
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
struct NakedSwordsman {

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

//impl ReflectDefault for UnitActionTuple {
//    fn reflect_default() -> Self {
//        // return the default value of UnitActionTuple
//        UnitActionTuple(UnitAction::DoNothing, 0.0)
//    }
//}

#[derive(Component)]
struct TalkUI {

}

#[derive(Component)]
struct Cursor {
	x: usize,
	y: usize,
}

#[derive(Component)]
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

#[derive(Component, Clone, Reflect, Default)]
#[reflect(Default)]
struct Pos {
	x: usize,
	y: usize,
}

#[derive(Component, Clone, Serialize, Deserialize)]
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

#[derive(Component, Clone, Serialize, Deserialize, Debug)]
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
}

// STATES

#[derive(States, Debug, Clone, Eq, PartialEq, Hash, Default)]
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

#[derive(Resource, Default)]
struct Game {
	current_unit: usize,
}

#[derive(Resource, Default)]
struct ClientData {
	client_id: ClientId,
}

#[derive(Resource, Default)]
struct DemoData {
	current_unit: UnitId,
}

// Client & Server
fn main() {
	std::panic::set_hook(Box::new(custom_panic_hook));
	
    App::new()
		.add_plugins(
			DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Ars Militaris".into(),
                    ..default()
                }),
                ..default()
            })
            .set(LogPlugin {
				file_appender_settings: Some(FileAppenderSettings {
					prefix: String::from("amclient.log"),
					rolling: Rolling::Minutely,
					..default()
				}),
				..default()
            })
        )
		.add_plugin(QuinnetClientPlugin::default())
		.add_plugins(ConsolePlugin)
		.insert_resource(ConsoleConfiguration {
			// Override config here.
			keys: vec![ToggleConsoleKey::KeyCode(KeyCode::Backslash)],
			..Default::default()
		})
		.add_plugin(WorldInspectorPlugin::new())
		.register_type::<ConsoleConfiguration>()
		.register_type::<HPCurrent>()
		.register_type::<UnitActions>()
		.register_type::<UnitAction>()
		.register_type::<UnitActionTuple>()
		.register_type::<Pos>()
		.add_plugin(ResourceInspectorPlugin::<ConsoleConfiguration>::default())
		.add_console_command::<DoNothingCommand, _>(do_nothing_command)
		.add_console_command::<TalkCommand, _>(talk_command)
		.add_console_command::<MoveCommand, _>(move_command)
		.add_state::<GameState>()
		.add_event::<GameStartEvent>()
		.add_event::<MapReadEvent>()
		.add_event::<MapSetupEvent>()
		.add_event::<UnitsReadEvent>()
		.add_event::<UnitsGeneratedEvent>()
		.init_resource::<Game>()
		.init_resource::<ClientData>()
		.init_resource::<DemoData>()
		.add_systems(Startup, set_window_icon)
		.add_systems(OnEnter(GameState::MainMenu),
			(start_connection, send_get_client_id_message)
				.chain()
		)
		.add_systems(Update,
			(send_start_game_message_system, handle_server_messages)
				.run_if(in_state(GameState::MainMenu))
		)
		.add_systems(Update,
			//(read_map_system, setup_map_system, read_battle_system, generate_units_system,  place_units_on_map_system, handle_player_turn_server_message)
			(read_map_system, setup_map_system, read_battle_system, (generate_units_system, apply_deferred).chain(),  place_units_on_map_system)
				.run_if(in_state(GameState::Loading))
		)
		.add_systems(OnExit(GameState::Loading), init_cursor_system)
		.add_systems(OnExit(GameState::Loading), setup_game_resource_system)
		.add_systems(OnEnter(GameState::LoadingComplete), loading_complete)
		.add_systems(OnEnter(GameState::Battle), (apply_deferred, setup_cursor_system).chain())
		.add_systems(Update,
			(move_cursor_system, end_turn_system, (apply_state_transition::<GameState>, handle_player_turn_server_message, apply_state_transition::<GameState>).chain())
				.run_if(in_state(GameState::Battle))
		)
		//.add_systems(Update, (apply_state_transition::<GameState>, current_state).chain().after(handle_player_turn_server_message).run_if(in_state(GameState::Wait)))
		//.add_systems(Update, (apply_state_transition::<GameState>, current_state).chain().run_if(in_state(GameState::Wait)))
		//.add_systems(Update, (apply_state_transition::<GameState>, current_state2).chain().run_if(in_state(GameState::Battle)))
		//.add_systems(Update, change_state.run_if(in_state(GameState::Battle)))
		.add_systems(OnExit(GameState::Battle), remove_cursor_system)
		//.add_systems(Update,
		//	(wait_turn_system, handle_player_turn_server_message)
		//		.run_if(in_state(GameState::WaitTurn))
		//)
		.add_systems(Update, handle_player_turn_server_message
			.run_if(in_state(GameState::Wait))
		)
		.add_systems(OnEnter(GameState::LoadAmbush), setup_grid_system)
		.add_systems(OnEnter(GameState::LoadAmbush), setup_camera_system)
		.add_systems(OnEnter(GameState::LoadAmbush), (apply_deferred, setup_text_system)
			.chain()
			.after(setup_grid_system)
		)
		.add_systems(OnEnter(GameState::LoadAmbush), (apply_deferred, spawn_units)
			.chain()
			.after(setup_text_system)
		)
		.add_systems(OnEnter(GameState::LoadAmbush), (apply_deferred, z_order_system)
			.chain()
			.after(setup_text_system)
		)
		.add_systems(OnEnter(GameState::LoadAmbush), (apply_deferred, z_unit_order_system)
			.after(spawn_units)
		)
//		.add_systems(Update, z_order_system
//			.run_if(in_state(GameState::Move))
//		)
//		.add_systems(Update, z_unit_order_system
//			.run_if(in_state(GameState::Move))
//		)
		.add_systems(Update, z_order_system
			.run_if(in_state(GameState::Ambush))
		)
		.add_systems(Update, z_unit_order_system
			.run_if(in_state(GameState::Ambush))
		)
		.add_systems(Update, tick_move_timer
			.run_if(in_state(GameState::Move))
		)
		.add_systems(Update, move_camera_system
			.run_if(in_state(GameState::Ambush))
		)
		.add_systems(Update, move_camera_system
			.run_if(in_state(GameState::Move))
		)
		//.add_systems(OnEnter(GameState::Ambush), ars_militaris_demo)
		.add_systems(Update, single_player_pause)
		.add_systems(Update, handle_single_player_pause_state
			.run_if(in_state(GameState::SinglePlayerPause))
		)
		//.add_systems(OnEnter(GameState::LoadMap), (apply_deferred, spawn_gaul_warrior)
		//	.chain()
		//	.after(setup_text_system)
		//)
		//.add_systems(OnEnter(GameState::Ambush), (apply_deferred, spawn_naked_swordsman)
		//	.chain()
		//	//.after(spawn_gaul_warrior)
		//	.after(setup_text_system)
		//)
		.add_systems(Update, move_gaul_warrior
			.run_if(in_state(GameState::Ambush))
			//.run_if(warrior_already_spawned)
		)
		.add_systems(Update, (process_unit_actions, apply_deferred)
			.chain()
			.run_if(in_state(GameState::Ambush))
			//.run_if(warrior_already_spawned)
		)
		//.add_systems(Update, (apply_deferred, first_move_unit_action)
		//	.chain()
		//	.run_if(in_state(GameState::Ambush).and_then(run_once()))
		//	.after(spawn_naked_swordsman)
		//)
		.add_systems(Update, (apply_deferred, process_move_actions, apply_deferred)
			.chain()
			.run_if(in_state(GameState::Ambush))
		)
		.add_systems(Update, handle_move_state
			.run_if(in_state(GameState::Move))
		)
		.add_systems(Update, (apply_deferred, process_talk_actions, apply_deferred)
			.chain()
			.run_if(in_state(GameState::Ambush))
		)
		.add_systems(Update, handle_talk_state
			.run_if(in_state(GameState::Talk))
		)
		//.add_systems(Update, setup_two_seconds_timer
		//	.run_if(in_state(GameState::LoadMap).and_then(run_once()))
		//	//.after(first_move_unit_action)
		//)
		//.add_systems(OnEnter(GameState::Ambush), (apply_deferred, cutscene_1)
		//	.chain()
		//	.after(spawn_naked_swordsman)
		//)
		//.add_systems(Update, second_move_action
		//	.run_if(in_state(GameState::LoadMap))
		//	.run_if(two_seconds_have_passed)
		//	.after(first_move_unit_action)
		//)
		//.add_systems(Update, (apply_deferred, first_talk_unit_action)
		//	.chain()
		//	.run_if(in_state(GameState::LoadMap).and_then(run_once()))
		//	//.after(third_move_action)
		//)
		//.add_systems(Update, third_move_action
		//	.run_if(six_seconds_have_passed)
		//	.after(second_move_action)
		//)
		//.add_systems(Update, (second_move_action.run_if(two_seconds_have_passed), third_move_action.run_if(six_seconds_have_passed), first_talk_unit_action.run_if(run_once()))
		//	.chain()
		//	.run_if(in_state(GameState::LoadMap))
		//	.after(first_move_unit_action)
		//)
		//.add_systems(Update, tick_timers
		//	.run_if(in_state(GameState::LoadMap))
		//)
		//.add_systems(Update, (apply_deferred, test_system_3)
		//	.chain()
		//	.run_if(in_state(GameState::LoadMap))
		//)
		//.add_systems(Update, print_gaul_warrior
		//	.run_if(in_state(GameState::LoadMap))
		//	.run_if(warrior_already_spawned)
		//)
		//.add_systems(Update, (apply_deferred, center_camera_on_unit)
		//	.chain()
		//	.run_if(in_state(GameState::LoadMap))
		//	.run_if(warrior_already_spawned)
		//)
		//.add_systems(Update, (apply_deferred, test_ortho_projection)
		//	.chain()
		//	.run_if(in_state(GameState::LoadMap))
		//)
		.add_systems(Startup, get_toggle_console_key)
		.run();
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
fn init_cursor_system(mut commands: Commands) {
	commands.spawn(Cursor {
		x: 0,
		y: 0,
	});
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
fn remove_cursor_system(cursors: Query<&Cursor>, mut tiles: Query<(&Tile, &Pos, &mut Text)>) {
	info!("DEBUG: Removing cursor...");
	for cursor in cursors.iter() {
		for (tile, pos, mut text) in tiles.iter_mut() {
			if pos.x == cursor.x && pos.y == cursor.y {
				// Remove cursor from tile.
				
				// Remove [ and ] from tile.
				let mut tile_string = &text.sections[0].value;
				let mut tile_string_split = tile_string.split("[");
				let vec = tile_string_split.collect::<Vec<&str>>();
				let mut tile_string_split_2 = vec[1].split("]");
				let vec2 = tile_string_split_2.collect::<Vec<&str>>();
				let new_tile_string = vec2[0];
				
				// Assign new string to tile.
				text.sections[0].value = new_tile_string.to_string();
			}
		}
	}
}

// Client
fn move_cursor_system(input: Res<Input<KeyCode>>, mut cursors: Query<&mut Cursor>, mut tiles: Query<(&Tile, &Pos, &mut Text)>) {
	
	// Get cursor current position.
	let mut cursor_position_x = 0;
	let mut cursor_position_y = 0;
	for cursor in cursors.iter_mut() {
		cursor_position_x = cursor.x;
		cursor_position_y = cursor.y;
	}
	
	if input.just_pressed(KeyCode::A) {
		
		// Save the previous cursor position to later be used in removing the cursor.
		let cursor_previous_x = cursor_position_x;
		let cursor_previous_y = cursor_position_y;
		
		if cursor_position_y == 9 {
			info!("DEBUG: You can't move the cursor there.");
		} else {
			
		
			// Find tile to the left of cursor.
			for (tile, pos, mut text) in tiles.iter_mut() {
				if pos.x == cursor_position_x && pos.y == cursor_position_y + 1 {
					// Move the cursor to the new position.
					info!("DEBUG: Found tile at coordinates {}, {}.", pos.x, pos.y);
					
					// Build cursor string.
					let mut cursor_string = "[".to_owned();
					cursor_string.push_str(&text.sections[0].value);
					cursor_string.push_str("]");
					text.sections[0].value = cursor_string;
					
					
					
					
					// Update the cursor Entity.
					for mut cursor in cursors.iter_mut() {
						cursor.x = pos.x;
						cursor.y = pos.y;

					}
				}
			}
			
			// Remove the cursor from the previous tile.
			for cursor in cursors.iter_mut() {
				for (tile, pos, mut text) in tiles.iter_mut() {
					if pos.x == cursor_previous_x && pos.y == cursor_previous_y {
						// Remove [ and ] from tile.
						let mut tile_string = &text.sections[0].value;
						let mut tile_string_split = tile_string.split("[");
						let vec = tile_string_split.collect::<Vec<&str>>();
						let mut tile_string_split_2 = vec[1].split("]");
						let vec2 = tile_string_split_2.collect::<Vec<&str>>();
						let new_tile_string = vec2[0];
						
						// Assign new string to tile.
						text.sections[0].value = new_tile_string.to_string();
					}
				}
			}
			
			info!("DEBUG: Moving the cursor...");
			
		}
	} else if input.just_pressed(KeyCode::D) {
	
		// Save the previous cursor position to later be used in removing the cursor.
		let cursor_previous_x = cursor_position_x;
		let cursor_previous_y = cursor_position_y;
		
		if cursor_position_y == 0 {
			info!("DEBUG: You can't move the cursor there.");
		} else {
			
		
			// Find tile to the left of cursor.
			for (tile, pos, mut text) in tiles.iter_mut() {
				if pos.x == cursor_position_x && pos.y == cursor_position_y - 1 {
					// Move the cursor to the new position.
					info!("DEBUG: Found tile at coordinates {}, {}.", pos.x, pos.y);
					
					// Build cursor string.
					let mut cursor_string = "[".to_owned();
					cursor_string.push_str(&text.sections[0].value);
					cursor_string.push_str("]");
					text.sections[0].value = cursor_string;
					
					
					
					
					// Update the cursor Entity.
					for mut cursor in cursors.iter_mut() {
						cursor.x = pos.x;
						cursor.y = pos.y;

					}
				}
			}
			
			// Remove the cursor from the previous tile.
			for cursor in cursors.iter_mut() {
				for (tile, pos, mut text) in tiles.iter_mut() {
					if pos.x == cursor_previous_x && pos.y == cursor_previous_y {
						// Remove [ and ] from tile.
						let mut tile_string = &text.sections[0].value;
						let mut tile_string_split = tile_string.split("[");
						let vec = tile_string_split.collect::<Vec<&str>>();
						let mut tile_string_split_2 = vec[1].split("]");
						let vec2 = tile_string_split_2.collect::<Vec<&str>>();
						let new_tile_string = vec2[0];
						
						// Assign new string to tile.
						text.sections[0].value = new_tile_string.to_string();
					}
				}
			}
			
			info!("DEBUG: Moving the cursor...");
			
		}
	} else if input.just_pressed(KeyCode::W) {
	
		// Save the previous cursor position to later be used in removing the cursor.
		let cursor_previous_x = cursor_position_x;
		let cursor_previous_y = cursor_position_y;
		
		if cursor_position_x == 0 {
			info!("DEBUG: You can't move the cursor there.");
		} else {
			
		
			// Find tile to the left of cursor.
			for (tile, pos, mut text) in tiles.iter_mut() {
				if pos.x == cursor_position_x - 1 && pos.y == cursor_position_y {
					// Move the cursor to the new position.
					info!("DEBUG: Found tile at coordinates {}, {}.", pos.x, pos.y);
					
					// Build cursor string.
					let mut cursor_string = "[".to_owned();
					cursor_string.push_str(&text.sections[0].value);
					cursor_string.push_str("]");
					text.sections[0].value = cursor_string;
					
					
					
					
					// Update the cursor Entity.
					for mut cursor in cursors.iter_mut() {
						cursor.x = pos.x;
						cursor.y = pos.y;

					}
				}
			}
			
			// Remove the cursor from the previous tile.
			for cursor in cursors.iter_mut() {
				for (tile, pos, mut text) in tiles.iter_mut() {
					if pos.x == cursor_previous_x && pos.y == cursor_previous_y {
						// Remove [ and ] from tile.
						let mut tile_string = &text.sections[0].value;
						let mut tile_string_split = tile_string.split("[");
						let vec = tile_string_split.collect::<Vec<&str>>();
						let mut tile_string_split_2 = vec[1].split("]");
						let vec2 = tile_string_split_2.collect::<Vec<&str>>();
						let new_tile_string = vec2[0];
						
						// Assign new string to tile.
						text.sections[0].value = new_tile_string.to_string();
					}
				}
			}
			
			info!("DEBUG: Moving the cursor...");
			
		}
	} else if input.just_pressed(KeyCode::S) {
	
		// Save the previous cursor position to later be used in removing the cursor.
		let cursor_previous_x = cursor_position_x;
		let cursor_previous_y = cursor_position_y;
		
		if cursor_position_x == 9 {
			info!("DEBUG: You can't move the cursor there.");
		} else {
			
		
			// Find tile to the left of cursor.
			for (tile, pos, mut text) in tiles.iter_mut() {
				if pos.x == cursor_position_x + 1 && pos.y == cursor_position_y {
					// Move the cursor to the new position.
					info!("DEBUG: Found tile at coordinates {}, {}.", pos.x, pos.y);
					
					// Build cursor string.
					let mut cursor_string = "[".to_owned();
					cursor_string.push_str(&text.sections[0].value);
					cursor_string.push_str("]");
					text.sections[0].value = cursor_string;
					
					
					
					
					// Update the cursor Entity.
					for mut cursor in cursors.iter_mut() {
						cursor.x = pos.x;
						cursor.y = pos.y;

					}
				}
			}
			
			// Remove the cursor from the previous tile.
			for cursor in cursors.iter_mut() {
				for (tile, pos, mut text) in tiles.iter_mut() {
					if pos.x == cursor_previous_x && pos.y == cursor_previous_y {
						// Remove [ and ] from tile.
						let mut tile_string = &text.sections[0].value;
						let mut tile_string_split = tile_string.split("[");
						let vec = tile_string_split.collect::<Vec<&str>>();
						let mut tile_string_split_2 = vec[1].split("]");
						let vec2 = tile_string_split_2.collect::<Vec<&str>>();
						let new_tile_string = vec2[0];
						
						// Assign new string to tile.
						text.sections[0].value = new_tile_string.to_string();
					}
				}
			}
			
			info!("DEBUG: Moving the cursor...");
			
		}
	}
}

// Server
fn wait_turn_system(mut units: Query<(&mut WTCurrent, &WTMax, &UnitId)>, mut game: ResMut<Game>, mut commands: Commands, client: Res<Client>) {
	
	// Decrease all units WT. If WT equals 0, set the unit as the current unit turn.
	for (mut wt_current, wt_max, unit_id) in units.iter_mut() {
		if wt_current.value == 0 {
			if game.current_unit == unit_id.value {
				break;
			} else {
				game.current_unit = unit_id.value;
			}
			
			info!("DEBUG: It is now unit {} turn.", unit_id.value);
			// Send WaitTurnComplete message.
			info!("DEBUG: Sending WaitTurnComplete message..."); 
			client
				.connection()
				.try_send_message(ClientMessage::WaitTurnComplete);
			info!("DEBUG: Sent WaitTurnComplete message.");
			
			//info!("DEBUG: Setting GameState to Wait..."); 
			//commands.insert_resource(NextState(GameState::Wait));
			//info!("DEBUG: Set GameState to Wait.");
		} else {
			wt_current.value = wt_current.value - 1;
		}
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
	commands.insert_resource(Game {
		current_unit: 0,
	});
}

// Client
fn start_connection(mut client: ResMut<Client>) {
	client
		.open_connection(
			ConnectionConfiguration::from_strings(
				//"127.0.0.1:6000",
				"139.162.244.70:6000",
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
	mut units: Query<(&UnitId, &mut WTCurrent)>,
	mut game: ResMut<Game>,
	mut next_state: ResMut<NextState<GameState>>,
	state: Res<State<GameState>>,
) {
	while let Ok(Some(message)) = client.connection_mut().receive_message::<ServerMessage>() {
		match message {
			ServerMessage::PlayerTurn { client_id, current_unit } => {
				info!("DEBUG: Received PlayerTurn message.");
				info!("DEBUG: Current state is {:?}.", state.get());
				// Update Game resouce.
				info!("DEBUG: Setting current unit to {}.", current_unit);
				game.current_unit = current_unit;
				info!("DEBUG: Set current unit to {}.", game.current_unit);
				
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
				} else {
					// Set state to Wait.
					info!("DEBUG: Setting GameState to Wait...");
					//commands.insert_resource(NextState(GameState::Wait));
					next_state.set(GameState::Wait);
					info!("DEBUG: Set GameState to Wait.");
					info!("DEBUG: Current state is {:?}.", state.get());
				}
			},
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
			ServerMessage::Wait => {
				// Set state to Wait.
				info!("DEBUG: Received Wait message.");
				info!("DEBUG: Setting GameState to Wait...");
				//commands.insert_resource(NextState(GameState::Wait));
				next_state.set(GameState::Wait);
				info!("DEBUG: Set GameState to Wait.");
				info!("DEBUG: Current state is {:?}.", state.get());
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
//
//// Client
//fn spawn_gaul_warrior(
//mut commands: Commands,
//mut map_query: Query<&mut Map>,
//asset_server: Res<AssetServer>,
//) {
//	info!("DEBUG: Spawning Gaul Warrior...");
//	
//	let mut map = &mut map_query.single_mut().map;
//	
//	let entity_id = commands.spawn((
//		SpriteBundle {
//			texture: asset_server.load("gaul_warrior.png"),
//			transform: Transform::from_xyz((0 as f32) * 256.0 / 2.0 - (0 as f32) * 256.0 / 2.0, (0 as f32) * 128.0 / 2.0 + (0 as f32) * 128.0 / 2.0 + (map[0][0].0 as f32) * 15.0 + 100.0, 1.0),
//			..default()
//		},
//		Unit,
//		Pos {
//			x: 0,
//			y: 0,
//		},
//		UnitActions { unit_actions: Vec::<(UnitAction, f32)>::new(), processing_unit_action: false, },
//	)).id();
//	
//	map[0][0].2.push(entity_id);
//	
//	info!("DEBUG: Spawned Gaul Warrior.");
//}
//
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
//
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
//
//// Utility
//fn print_gaul_warrior(
//query: Query<&Transform, With<Unit>>,
//) {
//	let unit_transform = query.single();
//	info!("DEBUG: Unit translation is: {:?}.", unit_transform.translation);
//}
//
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

//// Client
//fn warrior_already_spawned(query: Query<&Unit>) -> bool {
//	if query.iter().len() == 0 {
//		return false;
//	} else {
//		return true;
//	}
//}
//
//// Client
//fn test_system(text: Query<&Transform, With<GameText>>) {
//	info!("DEBUG: Text is at position {}.", text.single().translation);
//}
//
//// Client
//fn center_camera_on_unit(
//unit_transform_query: Query<&Transform, With<Unit>>,
//mut camera_transform_query: Query<&mut Transform, (With<Camera>, Without<Unit>)>,
//) {
//	let unit_transform = unit_transform_query.single();
//	let mut camera_transform = camera_transform_query.single_mut();
//	
//	camera_transform.translation = Vec3::new(unit_transform.translation.x, unit_transform.translation.y, camera_transform.translation.z);
//}
//
//fn test_ortho_projection(
//ortho_projection_query: Query<&OrthographicProjection>,
//) {
//	let ortho_projection = ortho_projection_query.single();
//	
//	info!("DEBUG: Orthographic projection far is: {:?}.", ortho_projection.far);
//}
//
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
				UnitAction::DoNothing => {
					info!("DEBUG: Current unit action is DoNothing.");
					commands.entity(entity).insert(DoNothingAction);
				}
			}
		}
	}
}

//// Prototype
//fn first_move_unit_action(
//mut unit_query: Query<(Entity, &mut UnitActions), (With<Unit>, With<NakedSwordsman>)>,
//) {
//	for (entity, mut unit_actions) in unit_query.iter_mut() {
//		//unit_actions.unit_actions.0.push(UnitAction::Move { destination: Pos { x: 2, y: 2, } });
//		info!("DEBUG: Added Move UnitAction.");
//		info!("DEBUG: unit_actions length is now: {}.", unit_actions.unit_actions.len());
//	}
//}
//
//// Prototype
//fn first_talk_unit_action(
//mut unit_query: Query<(Entity, &mut UnitActions), With<Unit>>,
//) {
//	for (entity, mut unit_actions) in unit_query.iter_mut() {
//		//unit_actions.unit_actions.0.push(UnitAction::Talk { message: "Lorem ipsum".to_string() });
//		info!("DEBUG: Added Talk UnitAction.");
//		info!("DEBUG: unit_actions length is now: {}.", unit_actions.unit_actions.len());
//		break;
//	}
//}
//
// Prototype
fn process_move_actions(
mut commands: Commands,
mut map_query: Query<&mut Map>,
mut unit_query: Query<(Entity, &mut UnitActions, &mut Pos,  &MoveAction)>,
mut next_state: ResMut<NextState<GameState>>,
) {
	let map = &mut map_query.single_mut().map;
	
	for (entity, mut unit_actions, mut pos, move_action) in unit_query.iter_mut() {
		info!("DEBUG: Processing MoveAction...");
		info!("DEBUG: Move destination is: {}, {}.", move_action.destination.x, move_action.destination.y);
		
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
mut unit_query: Query<(Entity, &mut Transform, &mut UnitActions, &mut Pos, &mut MoveAction), Without<GameText>>,
tile_transform_query: Query<&Transform, (With<GameText>, Without<Unit>)>,
mut next_state: ResMut<NextState<GameState>>,
time: Res<Time>,
) {
	let map = &mut map_query.single_mut().map;

	if unit_query.iter_mut().len() == 0 {
		info!("DEBUG: No MoveActions remaining. Setting GameState to Ambush.");
		next_state.set(GameState::Ambush);
		info!("DEBUG: No MoveActions remaining. Set GameState to Ambush.");
	} else {
	
		for (entity, mut transform, mut unit_actions, mut pos, mut move_action) in unit_query.iter_mut() {
			if map[move_action.destination.x][move_action.destination.y].2.len() > 0 {
				info!("DEBUG: Couldn't move unit. There's an unit already there.");
				unit_actions.processing_unit_action = false;
				unit_actions.unit_actions.remove(0);
				commands.entity(entity).remove::<MoveAction>();
				info!("DEBUG: Processed MoveAction.");
			} else {
				
				if move_action.timer.just_finished() {
					// Complete processing of MoveAction.
					
					info!("DEBUG: Completing processing of MoveAction...");
					map[move_action.destination.x][move_action.destination.y].2.push(entity);
					map[pos.x][pos.y].2.pop();
					
					pos.x = move_action.destination.x;
					pos.y = move_action.destination.y;
					
					unit_actions.processing_unit_action = false;
					unit_actions.unit_actions.remove(0);
					commands.entity(entity).remove::<MoveAction>();
					info!("DEBUG: Processed MoveAction.");
					
	//				// Set GameState back to Ambush
	//				info!("DEBUG: Setting GameState to Ambush...");
	//				next_state.set(GameState::Ambush);
	//				info!("DEBUG: Set GameState to Ambush.");
				
				} else {
					// Move is still in progress, update the unit's position gradually
					//info!("DEBUG: Move in progress, updating unit position.");
					//move_action.timer.tick(time.delta());
					
					let start_pos = &move_action.origin;
					let end_pos = &move_action.destination;
					let progress = move_action.timer.elapsed_secs() / 4.0;

					

//					// NEED TO GET THE TRANSFORM AT THE START POS.
//
//					let new_pos_x = (start_pos.x as f32 * (1.0 - progress) + end_pos.x as f32 * progress) as f32;
//                    let new_pos_y = (start_pos.y as f32 * (1.0 - progress) + end_pos.y as f32 * progress) as f32;
//
//                    // Interpolate the translation between the old and new positions
//                    transform.translation.x = (new_pos_x * 256.0 / 2.0) - (new_pos_y as f32 * 256.0 / 2.0);
//                    transform.translation.y = (new_pos_x * 128.0 / 2.0)
//                        + (new_pos_y * 128.0 / 2.0)
//                        + (map[end_pos.x][end_pos.y].0 as f32) * 15.0 * (111.0 / (128.0 / 2.0) - 1.0)
//                        + 100.0;			
//					
//					
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

// Prototype
fn tick_move_timer(
mut move_action_query: Query<&mut MoveAction>,
time: Res<Time>
) {
	for (mut move_action) in move_action_query.iter_mut() {
		move_action.timer.tick(time.delta());
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

//
//// Prototype
//fn second_move_action(
//mut commands: Commands,
//mut swordsman_query: Query<(Entity, &mut UnitActions), With<NakedSwordsman>>,
//) {
//	info!("DEBUG: Adding second Move UnitAction...");
//	let (entity, mut unit_actions) = swordsman_query.single_mut();
//	//unit_actions.unit_actions.0.push(UnitAction::Move { destination: Pos { x: 4, y: 2, } });
//	info!("DEBUG: Added second Move UnitAction.");
//}
//
//// Prototype
//fn third_move_action(
//mut commands: Commands,
//mut swordsman_query: Query<(Entity, &mut UnitActions), With<NakedSwordsman>>,
//) {
//	info!("DEBUG: Adding third Move UnitAction...");
//	let (entity, mut unit_actions) = swordsman_query.single_mut();
//	//unit_actions.unit_actions.0.push(UnitAction::Move { destination: Pos { x: 6, y: 2, } });
//	info!("DEBUG: Added third Move UnitAction.");
//}
//
//// Prototype
//fn setup_two_seconds_timer(
//mut timers: ResMut<Timers>,
//) {
//	timers.two_second_timer = Timer::from_seconds(4.0, TimerMode::Once);
//	timers.six_second_timer = Timer::from_seconds(6.0, TimerMode::Once);
//}
//
//// Prototype
//fn two_seconds_have_passed(
//timers: Res<Timers>,
//
//) -> bool {
//	if timers.two_second_timer.just_finished() {
//		return true;
//	} else {
//		return false;
//	}
//}
//
//// Prototype
//fn six_seconds_have_passed(
//timers: Res<Timers>,
//) -> bool {
//	return timers.six_second_timer.just_finished();
//}
//
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
			},
			Unit,
			UnitActions { unit_actions: Default::default(), processing_unit_action: false, },
			Pos {
				x: record[4].parse().unwrap(),
				y: record[5].parse().unwrap(),
			},
		)).id();
		
		let mut path_string: String = record[19].to_string();
		path_string.push_str("_east.png");
		//let path_buf = PathBuf::from(path_string);
		//let path = Path::new(&path_string);
		//let asset_path = AssetPath::new_ref(path, None);
		//let mut asset_path: AssetPath<'static> = AssetPath::from(String::from(record[19].to_string()));
		//let path_buf: PathBuf = asset_path.as_ref().into();
		//let modified_path = format!("{:?}{}", asset_path, "_east.png");
		//let modified_path = asset_path.to_string_lossy().to_string() + "_east.png";
		//let modified_path = path_buf.with_extension("").to_string_lossy().to_string() + "_east.png";
		//info!("DEBUG: Unit path is: {:?}.", asset_path);
		//let modified_path = asset_path.to_string() + "_east.png";
		//asset_path = AssetPath::from(modified_path);
		
//		commands.entity(entity_id).insert(SpriteBundle {
//					texture: asset_server.load(path_string),
//					transform: Transform::from_xyz((record[4].parse::<usize>().unwrap() as f32) * 256.0 / 2.0 - (record[5].parse::<usize>().unwrap() as f32) * 256.0 / 2.0, (record[4].parse::<usize>().unwrap() as f32) * 128.0 / 2.0 + (record[5].parse::<usize>().unwrap() as f32) * 128.0 / 2.0  * (111.0 / (128.0 / 2.0) - 1.0) + (map[record[4].parse::<usize>().unwrap() as usize][record[5].parse::<usize>().unwrap() as usize].0 as f32) * 15.0 + 100.0, 1.0),
//					..default()
//		},);
		
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
		
		info!("DEBUG: Unit transform is: {:?}.", Transform::from_xyz((record[4].parse::<usize>().unwrap() as f32) * 256.0 / 2.0 - (record[5].parse::<usize>().unwrap() as f32) * 256.0 / 2.0, (record[4].parse::<usize>().unwrap() as f32) * 128.0 / 2.0 + (record[5].parse::<usize>().unwrap() as f32) * 128.0 / 2.0 + (map[record[4].parse::<usize>().unwrap() as usize][record[5].parse::<usize>().unwrap() as usize].0 as f32) * 15.0 + 100.0, 1.0));
		//info!("DEBUG: 
		
		//* (111.0 / (128.0 / 2.0) - 1.0)
	}
	
	info!("DEBUG: Finished spawning units.");
	info!("DEBUG: Setting GameState to Ambush.");
	next_state.set(GameState::Ambush);
	info!("DEBUG: Set GameState to Ambush.");
}

//
//// Prototype
//fn tick_timers(mut timers: ResMut<Timers>, time: Res<Time>) {
//	timers.two_second_timer.tick(time.delta());
//	timers.six_second_timer.tick(time.delta());
//}
//

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

// Logging
fn custom_panic_hook(info: &std::panic::PanicInfo) {
    // Perform any necessary logging or error handling here
    error!("Panic occurred: {:?}", info);
}

fn empty_system() {

}