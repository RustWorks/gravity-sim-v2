use legion::prelude::*;

use ggez::{
    event,
    event::EventHandler,
    graphics,
    graphics::{Color, DrawMode},
    input,
    input::{
        keyboard::{KeyCode, KeyMods},
        mouse::MouseButton,
    },
    Context, GameResult,
};

use crate::physics::{
    apply_gravity, calc_collisions, do_physics, integrate_kinematics, integrate_positions,
};
use crate::resources::{MainIterations, MousePos, Resolution, StartPoint, DT};
use crate::trails::update_trails;
#[allow(unused_imports)]
use crate::{
    imgui_wrapper::*, new_body, new_preview, Body, Draw, Kinematics, Mass, Point, Position,
    Preview, Radius, Trail, Vector,
};

static TRAIL_COLOR: graphics::Color = graphics::Color::new(0.2, 0.35, 1.0, 1.0);

use std::collections::HashSet;

const CAMERA_SPEED: f32 = 1.5;

pub fn scale_pos(point: impl Into<Point>, coords: graphics::Rect, resolution: Vector) -> Point {
    let mut np: Point = point.into();
    np.x *= coords.w / resolution.x;
    np.y *= coords.h / resolution.y;
    np.x += coords.x;
    np.y += coords.y;
    np
}

pub struct MainState {
    pub universe: Universe,
    pub main_world: World,
    pub imgui_wrapper: ImGuiWrapper,
    pub hidpi_factor: f32,
    pub resolution: Vector,
    pub selected_entity: Option<Entity>,
    pub mass: f32,
    pub rad: f32,
    pub dt: f32,
    pub num_iterations: i32,
    pub creating: bool,
    pub start_point: Option<Point>,
    pub items_hovered: bool,
    pub paused: bool,
    pub preview_iterations: usize,
}

impl MainState {
    pub fn new(
        universe: Universe,
        main_world: World,
        imgui_wrapper: ImGuiWrapper,
        hidpi_factor: f32,
        resolution: Vector,
    ) -> Self {
        MainState {
            universe,
            main_world,
            imgui_wrapper,
            hidpi_factor,
            resolution,
            selected_entity: None,
            dt: 1.0,
            mass: 0.1,
            rad: 1.0,
            num_iterations: 1,
            creating: false,
            start_point: None,
            items_hovered: false,
            paused: false,
            preview_iterations: 25,
        }
    }
}

impl EventHandler for MainState {
    fn update(&mut self, ctx: &mut Context) -> GameResult {
        self.imgui_wrapper
            .sent_signals
            .clone()
            .drain(..)
            .for_each(|signal| match signal {
                UiSignal::Create => self.creating = !self.creating,
            });
        self.imgui_wrapper.sent_signals.clear();

        let mut offset: Vector = Vector::new(0.0, 0.0);
        if input::keyboard::is_key_pressed(ctx, KeyCode::Up)
            || input::keyboard::is_key_pressed(ctx, KeyCode::W)
        {
            offset.y -= CAMERA_SPEED;
        }
        if input::keyboard::is_key_pressed(ctx, KeyCode::Down)
            || input::keyboard::is_key_pressed(ctx, KeyCode::S)
        {
            offset.y += CAMERA_SPEED;
        }
        if input::keyboard::is_key_pressed(ctx, KeyCode::Left)
            || input::keyboard::is_key_pressed(ctx, KeyCode::A)
        {
            offset.x -= CAMERA_SPEED;
        }
        if input::keyboard::is_key_pressed(ctx, KeyCode::Right)
            || input::keyboard::is_key_pressed(ctx, KeyCode::D)
        {
            offset.x += CAMERA_SPEED;
        }
        if offset != [0.0, 0.0].into() {
            let mut screen_coordinates = ggez::graphics::screen_coordinates(ctx);
            let zoom = screen_coordinates.w / crate::SCREEN_X;

            screen_coordinates.x += offset.x * zoom;
            screen_coordinates.y += offset.y * zoom;

            ggez::graphics::set_screen_coordinates(ctx, screen_coordinates).unwrap_or(());
        }

        if ggez::timer::ticks(ctx) % 60 == 0 {
            dbg!(ggez::timer::fps(ctx));
        }

        if !self.paused {
            // for _ in 0..self.num_iterations {
            //     calc_collisions(&mut self.main_world, self.start_point, ctx, self.resolution);
            //     integrate_positions(&mut self.main_world, self.dt);
            //     apply_gravity(&mut self.main_world);
            //     integrate_kinematics(&mut self.main_world, self.dt);
            //     update_trails(&mut self.main_world);
            // }
            let mouse_pos = ggez::input::mouse::position(ctx);
            let coords = ggez::graphics::screen_coordinates(ctx);
            let mouse_pos = crate::main_state::scale_pos(mouse_pos, coords, self.resolution);

            self.main_world
                .resources
                .insert::<StartPoint>(StartPoint(self.start_point));
            self.main_world
                .resources
                .insert::<MousePos>(MousePos(mouse_pos));
            self.main_world
                .resources
                .insert::<Resolution>(Resolution(self.resolution));
            do_physics(&mut self.main_world, ctx);
        }

        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult {
        graphics::clear(ctx, Color::new(0.0, 0.0, 0.0, 1.0));

        let mut builder = graphics::MeshBuilder::new();

        let mut draw_query = <(Read<Draw>, Read<Position>, Read<Radius>)>::query();
        let mut draw_preview_query = <(Read<Preview>, Read<Position>, Read<Radius>)>::query();

        let mut trail_query = <(Read<Trail>, Read<Radius>)>::query();

        draw_query
            .iter(&mut self.main_world)
            .for_each(|(color, pos, rad)| {
                let point: ggez::mint::Point2<f32> = (*pos).into();
                builder.circle(DrawMode::fill(), point, rad.0, 0.01, color.0);
            });

        draw_preview_query
            .iter_immutable(&self.main_world)
            .for_each(|(_, pos, rad)| {
                let point: ggez::mint::Point2<f32> = (*pos).into();
                let color = Color::new(0.1, 1.0, 0.2, 1.0);
                builder.circle(DrawMode::fill(), point, rad.0, 0.05, color);
            });

        trail_query
            .iter_immutable(&self.main_world)
            .for_each(|(trail, radius)| {
                let slices = trail.0.as_slices();
                if slices.0.len() >= 2 {
                    if let Err(e) = builder.line(slices.0, 0.25 * radius.0, TRAIL_COLOR) {
                        dbg!(e);
                    };
                }
                if slices.1.len() >= 2 {
                    if let Err(e) = builder.line(slices.1, 0.25 * radius.0, TRAIL_COLOR) {
                        dbg!(e);
                    };
                }
            });

        let p = if let Some(start_pos) = self.start_point {
            start_pos
        } else {
            let mouse_pos = ggez::input::mouse::position(ctx);
            let coords = ggez::graphics::screen_coordinates(ctx);
            scale_pos(mouse_pos, coords, self.resolution)
        };

        if self.creating {
            builder.circle(
                DrawMode::fill(),
                p,
                self.rad,
                0.05,
                Color::new(1.0, 1.0, 1.0, 0.5),
            );

            if let Some(p) = self.start_point {
                let mouse_pos = ggez::input::mouse::position(ctx);
                let coords = ggez::graphics::screen_coordinates(ctx);
                let scaled_pos = scale_pos(mouse_pos, coords, self.resolution);
                builder
                    .line(&[p, scaled_pos], 0.5, graphics::WHITE)
                    .expect("not enough points in line");
            }
        }

        let mesh = builder.build(ctx).expect("error building mesh");

        // self.imgui_wrapper.shown_menus.push(UiChoice::DefaultUI);

        ggez::graphics::draw(ctx, &mesh, graphics::DrawParam::new()).expect("error drawing mesh");
        let hidpi_factor = self.hidpi_factor;
        let mut dt = self.main_world.resources.get_or_insert(DT(1.0)).unwrap().0;
        let mut main_iter = self
            .main_world
            .resources
            .get_or_insert(MainIterations(1))
            .unwrap()
            .0;

        if let Some(e) = self.selected_entity {
            let mut mass = self.main_world.get_component::<Mass>(e).unwrap().0;
            let mut rad = self.main_world.get_component::<Radius>(e).unwrap().0;

            if self.main_world.is_alive(e) {
                self.imgui_wrapper.render(
                    ctx,
                    hidpi_factor,
                    &mut dt,
                    &mut mass,
                    &mut rad,
                    &mut main_iter,
                    &mut self.items_hovered,
                    true,
                );
                self.main_world.get_component_mut::<Mass>(e).unwrap().0 = mass;
                self.main_world.get_component_mut::<Radius>(e).unwrap().0 = rad;
            } else {
                self.selected_entity = None;
            }
        } else {
            self.imgui_wrapper.render(
                ctx,
                hidpi_factor,
                &mut dt,
                &mut self.mass,
                &mut self.rad,
                &mut main_iter,
                &mut self.items_hovered,
                false,
            );
        }
        self.main_world
            .resources
            .insert::<MainIterations>(MainIterations(main_iter));
        self.main_world.resources.insert::<DT>(DT(dt));

        ggez::graphics::present(ctx)
    }

    fn mouse_button_down_event(
        &mut self,
        ctx: &mut Context,
        button: event::MouseButton,
        x: f32,
        y: f32,
    ) {
        self.imgui_wrapper.update_mouse_down((
            button == MouseButton::Left,
            button == MouseButton::Right,
            button == MouseButton::Middle,
        ));

        if !self.items_hovered {
            match button {
                MouseButton::Right => {
                    self.imgui_wrapper.shown_menus.clear();
                    let mut clicked_query = <(Read<Position>, Read<Radius>)>::query();
                    self.selected_entity = None;

                    let coords = ggez::graphics::screen_coordinates(ctx);
                    let mouse_pos = scale_pos([x, y], coords, self.resolution);

                    for (e, (pos, rad)) in clicked_query.iter_entities(&mut self.main_world) {
                        if pos.dist(mouse_pos) <= rad.0 {
                            self.selected_entity = Some(e);
                            break;
                        }
                    }

                    self.imgui_wrapper
                        .shown_menus
                        .push(UiChoice::SideMenu(self.selected_entity));
                }
                MouseButton::Left => {
                    if self.creating {
                        let p = Point::new(x, y);
                        let coords = ggez::graphics::screen_coordinates(ctx);
                        self.start_point = Some(scale_pos(p, coords, self.resolution));

                        self.main_world
                            .insert((), vec![new_preview(p, [0.0, 0.0], self.rad)]);
                    }
                }
                _ => {}
            }
        }
    }

    fn mouse_button_up_event(
        &mut self,
        ctx: &mut Context,
        button: event::MouseButton,
        x: f32,
        y: f32,
    ) {
        self.imgui_wrapper.update_mouse_down((false, false, false));
        if let Some(start_point) = self.start_point {
            match button {
                MouseButton::Left => {
                    if self.creating && !self.imgui_wrapper.sent_signals.contains(&UiSignal::Create)
                    {
                        let mut p = Point::new(x, y);
                        let coords = ggez::graphics::screen_coordinates(ctx);
                        p = scale_pos(p, coords, self.resolution);

                        self.main_world.insert(
                            (),
                            vec![new_body(
                                start_point,
                                (start_point - p) * 0.10,
                                self.mass,
                                self.rad,
                            )],
                        );
                        self.start_point = None;
                    }
                }
                _ => dbg!(),
            }
        }

        let mut preview_query = <(Read<Preview>)>::query();
        let mut delset: HashSet<Entity> = HashSet::new();

        preview_query
            .iter_entities(&mut self.main_world)
            .for_each(|(entity, _)| {
                delset.insert(entity);
            });

        delset.iter().for_each(|entity| {
            self.main_world.delete(*entity);
        });
    }

    fn mouse_motion_event(&mut self, ctx: &mut Context, x: f32, y: f32, dx: f32, dy: f32) {
        self.imgui_wrapper.update_mouse_pos(x, y);

        let mut preview_query = <(Read<Preview>)>::query();
        let mut delset: HashSet<Entity> = HashSet::new();

        preview_query
            .iter_entities(&mut self.main_world)
            .for_each(|(entity, _)| {
                delset.insert(entity);
            });

        delset.iter().for_each(|entity| {
            self.main_world.delete(*entity);
        });

        let mut coords = ggez::graphics::screen_coordinates(ctx);
        if let Some(start_point) = self.start_point {
            let p = scale_pos([x, y], coords, self.resolution);

            self.main_world.insert(
                (),
                vec![new_preview(start_point, (start_point - p) * 0.1, self.rad)],
            );
        }

        if input::mouse::button_pressed(ctx, input::mouse::MouseButton::Middle) {
            let mut offset = Vector::new(dx, dy);
            offset.x *= coords.w / self.resolution.x;
            offset.y *= coords.h / self.resolution.y;
            coords.x -= offset.x;
            coords.y -= offset.y;
            graphics::set_screen_coordinates(ctx, coords).expect("error moving my mclick");
        }
    }

    fn mouse_wheel_event(&mut self, ctx: &mut Context, _x: f32, y: f32) {
        let mouse_pos = input::mouse::position(ctx);
        let mut offset = graphics::screen_coordinates(ctx);

        let prev_zoom = offset.w / crate::SCREEN_X;
        let zoom = prev_zoom * (1.0 - (y * 0.05));

        let focus: Vector = Vector::new(mouse_pos.x, mouse_pos.y);

        let mut scaled_focus1: Vector = focus;
        scaled_focus1.x *= offset.w / self.resolution.x;
        scaled_focus1.y *= offset.h / self.resolution.y;

        offset.w = zoom * crate::SCREEN_X;
        offset.h = zoom * crate::SCREEN_Y / (self.resolution.x / self.resolution.y);

        let mut scaled_focus2: Vector = focus;
        scaled_focus2.x *= offset.w / self.resolution.x;
        scaled_focus2.y *= offset.h / self.resolution.y;

        let delta_focus = scaled_focus2 - scaled_focus1;

        offset.x -= delta_focus.x;
        offset.y -= delta_focus.y;

        graphics::set_screen_coordinates(ctx, offset).unwrap_or(());
    }

    fn key_down_event(
        &mut self,
        _ctx: &mut Context,
        keycode: KeyCode,
        _keymods: KeyMods,
        _repeat: bool,
    ) {
        #[allow(clippy::single_match)]
        match keycode {
            KeyCode::Space => self.paused = !self.paused,
            KeyCode::Escape => self.imgui_wrapper.shown_menus.clear(),
            _ => {}
        };
    }

    fn resize_event(&mut self, ctx: &mut Context, width: f32, height: f32) {
        let aspect_ratio = height / width;
        graphics::set_screen_coordinates(
            ctx,
            graphics::Rect::new(
                0.,
                0.,
                crate::SCREEN_X,
                crate::SCREEN_Y * aspect_ratio as f32,
            ),
        )
        .expect("error resizing");
        let resolution = Vector::new(width, height);
        self.imgui_wrapper.resolution = resolution;
        self.resolution = resolution;
    }
}
