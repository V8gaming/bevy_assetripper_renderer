use std::time::Instant;

use bevy::{
    prelude::*, 
    render::camera::Viewport, 
    window::WindowResized, 
    winit::WinitSettings,
};
use bevy_atmosphere::prelude::*;
use bevy_spectator::*;


mod mat_gen;
use crate::mat_gen::Materials;
fn main() {
    let _ = Materials::from_dir("./Assets", "./assets/Assets").run();
    App::new()
    .insert_resource(AtmosphereModel::new(Gradient{
        sky: Color::rgb_u8(135, 206, 235),
        horizon: Color::rgb_u8(135, 206, 235),
        ground: Color::rgb_u8(135, 206, 235),
    }))
    .insert_resource(WinitSettings::desktop_app())
    .add_systems(Startup, setup)
    .add_systems(Update,spin)
    .add_systems(Update, scroll)
    .add_systems(Update, change_asset)
    .add_plugins((DefaultPlugins, AtmospherePlugin, SpectatorPlugin))

    .run();
 
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    window: Query<&Window>,

) {
    // Load the texture files
    let albedo = asset_server.load("textures/T_Apple_01_A_ALB.png");
    let normal = asset_server.load("textures/T_Apple_01_A_NRM.png");
    let orm = asset_server.load("textures/T_Apple_01_A_ORM.png");
    // Create a material from the textures
    let material = materials.add(StandardMaterial {
        base_color_texture: Some(albedo.clone()),
        normal_map_texture: Some(normal.clone()),
        metallic_roughness_texture: Some(orm.clone()),
        occlusion_texture: Some(orm.clone()),
        unlit: true,
    
        ..Default::default()
    });
    let grid = asset_server.load("textures/Grid.jpg");

    // Spawn a mesh with the material
    commands
        .spawn((PbrBundle {
            mesh: asset_server.load("mesh/SM_Apple_01_A.glb#Mesh0/Primitive0"),
            material: material.clone(),
            transform: Transform::from_scale(Vec3::splat(5.0)),
            ..Default::default()
        },
        Spin{},
        AssetData {
            mesh: "mesh/SM_Apple_01_A.glb#Mesh0/Primitive0".to_string(),
            albedo: "textures/T_Apple_01_A_ALB.png".to_string(),
            normal: "textures/T_Apple_01_A_NRM.png".to_string(),
            orm: "textures/T_Apple_01_A_ORM.png".to_string(),
            material_id: material.clone(),
        }
    ));
    

    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Mesh::from(shape::Plane { size: 100.0, subdivisions: 100})),
            material: materials.add(StandardMaterial {
                base_color_texture: Some(grid.clone()),
                unlit: true,
                ..Default::default()
            }),
            transform: Transform::from_translation(Vec3::new(0.0, -5.0, 0.0)),
            ..Default::default()
        },
    ));
    // create ui in left third of screen
    // a scrollable list of different assets, Vec<(PATH, NAME)>
    let mut list: Vec<(String, String)> = std::fs::read_dir("./assets/Assets/assets").unwrap().map(|x| {
        let path = x.unwrap().path();
        let name = path.file_name().unwrap().to_str().unwrap().to_string();
        (path.to_str().unwrap().strip_prefix("./assets/").unwrap().to_string(), name)
    }).collect();

    // Sort the list alphabetically
    list.sort_by(|a, b| a.1.cmp(&b.1));

    let style = TextStyle {
        font: asset_server
            .load("fonts/FiraCode-Regular.ttf"),
        font_size: 20.,
        ..default()
    };
    commands.spawn(NodeBundle {
        style: Style {
            width: Val::Percent(33.0),
            height: Val::Percent(100.0),
            overflow: Overflow::clip(),
            align_self: AlignSelf::Start,
            ..Default::default()
        },
        ..Default::default()
    }).with_children(|parent| {
        parent.spawn((
            TextBundle::from_section("", style.clone()),
            VisibleItems {
                items: list.clone(),
                visible: list,
                selected: 0,
                last_selected: None,
                offset: 0,
            },
            LastPressedTime {
                time: std::time::Instant::now(),
            }
    ));
    });

    spawn_camera(commands, window);
    
}


/// Spawn a camera like this
fn spawn_camera(mut commands: Commands, window: Query<&Window>) {
    let translation = Vec3::new(-2.0, 2.5, 5.0);
    let window = window.single();
    let window_size: (f32, u32) = (window.physical_width() as f32, window.physical_height());
    let third_x = window_size.0 / 3.0;
    commands.spawn((
        Camera3dBundle {
            camera: Camera {
                viewport: Some(Viewport {
                    physical_position: UVec2::new(third_x as u32, 0),
                    physical_size: UVec2::new((third_x*2.0) as u32, window_size.1),
                    ..Default::default()
                    
                }),

                ..Default::default()
            },
            transform: Transform::from_translation(translation)
                .looking_at(Vec3::ZERO, Vec3::Y),
            ..Default::default()
        },
        AtmosphereCamera::default(),
        Spectator,
    ));
}

#[derive(Component)]
struct Spin;

fn spin(time: Res<Time>, mut query: Query<(&Spin, &mut Transform)>) {
    for (_spin, mut transform) in query.iter_mut() {
        transform.rotate(Quat::from_rotation_y(time.delta_seconds()));
    }
}

#[derive(Component)]
struct VisibleItems {
    items: Vec<(String, String)>,
    visible: Vec<(String, String)>,
    selected: usize,
    last_selected: Option<usize>,
    offset: usize,
}
#[derive(Component)]
struct LastPressedTime {
    time: Instant,

}
fn scroll(
    window: Query<&Window>,
    mut query_list: Query<(&mut VisibleItems, &mut Text, &mut LastPressedTime)>,
    keys: Res<Input<KeyCode>>,
) {
    let window = window.single();
    let window_size = (window.physical_width() as usize, window.physical_height()as usize);
    
    for (mut data, mut text, mut time) in &mut query_list.iter_mut() {
        let max: usize = window_size.1 / text.sections[0].style.font_size as usize;
        
        if time.time.elapsed().as_millis() < 100 {
            continue;
        }
        time.time = Instant::now();
        if keys.pressed(KeyCode::Up) {
            if data.selected > 0 {
                data.selected -= 1;
            } else if data.offset > 0 {
                data.offset -= 1;
            }
        } else if keys.pressed(KeyCode::Down) {
            if data.selected < max - 1 && data.selected < data.items.len() - 1 {
                data.selected += 1;
            } else if data.offset < data.items.len() - max {
                data.offset += 1;
            }
        } else if keys.pressed(KeyCode::PageDown) {
            if data.selected < data.items.len() - max {
                data.selected += max;
            } else {
                data.selected = data.items.len() - 1;
            }
            if data.offset < data.items.len() - max {
                data.offset += max;
            } else {
                data.offset = data.items.len() - max;
            }
        } else if keys.pressed(KeyCode::PageUp) {
            if data.selected > max {
                data.selected -= max;
            } else {
                data.selected = 0;
            }
            if data.offset > max {
                data.offset -= max;
            } else {
                data.offset = 0;
            }
        } else if keys.just_pressed(KeyCode::Home) {
            data.selected = 0;
            data.offset = 0;
        } 

        let items: Vec<(String, String)> = data.items.iter().skip(data.offset).take(max).cloned().collect();
        data.visible = items;
        let mut sections = String::new();
        for (i, (_path, name)) in data.visible.clone().iter().enumerate() {
            let mut text = name.clone();
            if i == data.selected {
                text = format!("> {}", name);
            }
            sections.push_str(&format!("{}\n", text));
        }
        text.sections[0].value = sections;

    }
}

#[derive(Component)]
struct AssetData {
    mesh: String,
    albedo: String,
    normal: String,
    orm: String,
    material_id: Handle<StandardMaterial>,
} 

fn change_asset(
    current : Query<&mut VisibleItems>,
    mut asset: Query<(&mut AssetData, &mut Handle<Mesh>)>,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>

) {
    let current = current.get_single().unwrap();
    if current.last_selected.is_some() && current.selected == current.last_selected.unwrap() {
        return;
    }
    let current = current.items[current.selected+current.offset].clone();
    let (mut asset, mut mesh) = asset.get_single_mut().unwrap();
    let path = current.0;
    let mut exists = (false,false, false, false);

    for file in std::fs::read_dir(format!("./assets/{}",path)).unwrap() {
        let path = file.unwrap().path();
        let changed_path = path.to_str().unwrap().strip_prefix("./assets/").unwrap();
        let name = path.file_name().unwrap().to_str().unwrap().to_string();
        //println!("{}", changed_path);
        if name.ends_with("glb") {
            let asset_handle: Handle<Mesh> = asset_server.load(format!("{}#Mesh0/Primitive0",changed_path));
            asset.mesh = changed_path.to_string();
            *mesh = asset_handle;
            exists.0 = true;
        } else if name.ends_with("BaseColor.png") {
            let asset_handle: Handle<Image> = asset_server.load(changed_path);
            asset.albedo = changed_path.to_string();
            materials.get_mut(&asset.material_id).unwrap().base_color_texture = Some(asset_handle);
            exists.1 = true;
        } else if name.ends_with("Normal.png") {
            let asset_handle: Handle<Image> = asset_server.load(changed_path);
            asset.normal = changed_path.to_string();
            materials.get_mut(&asset.material_id).unwrap().normal_map_texture = Some(asset_handle);
            exists.2 = true;
        } else if name.ends_with("ORM.png") {
            let asset_handle: Handle<Image> = asset_server.load(changed_path);
            asset.orm = changed_path.to_string();
            materials.get_mut(&asset.material_id).unwrap().metallic_roughness_texture = Some(asset_handle.clone());
            materials.get_mut(&asset.material_id).unwrap().occlusion_texture = Some(asset_handle);
            exists.3 = true;
        }

    }
    if !exists.0 {
        *mesh = meshes.add(Mesh::from(shape::Torus {
            radius: 0.1,
            ring_radius: 0.03,
            subdivisions_segments: 100,
            subdivisions_sides: 100,
        
        }));
        asset.mesh = "mesh/Torus".to_string();
    }
    if !exists.1 {
        materials.get_mut(&asset.material_id).unwrap().base_color_texture = Some(asset_server.load("textures/The_Missing_textures.png"));
    }
    if !exists.2 {
        materials.get_mut(&asset.material_id).unwrap().normal_map_texture = None;
    }
    if !exists.3 {
        materials.get_mut(&asset.material_id).unwrap().metallic_roughness_texture = None;
        materials.get_mut(&asset.material_id).unwrap().occlusion_texture = None;
    }
    asset_server.free_unused_assets();

}