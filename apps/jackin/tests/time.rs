#![cfg(feature = "fixtures")]

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use jackin_console::fixtures::HOME;
use jackin_console::scenario::{CLOCK, Scenario};
use jackin_console::sim::SimBackend;
use jackin_console::sim::onepassword::SimOnePassword;
use jackin_console::world::{WallSource, World, WorldParts};

const NOON_UTC_MS: i64 = 1_788_436_800_000;

#[derive(Debug)]
struct FakeWall {
    epoch_ms: AtomicI64,
    offset: AtomicI64,
}

impl FakeWall {
    fn at(epoch_ms: i64, offset: i64) -> Arc<Self> {
        Arc::new(Self {
            epoch_ms: AtomicI64::new(epoch_ms),
            offset: AtomicI64::new(offset),
        })
    }

    fn advance(&self, ms: i64) {
        self.epoch_ms.fetch_add(ms, Ordering::SeqCst);
    }
}

impl WallSource for FakeWall {
    fn epoch_millis(&self) -> i64 {
        self.epoch_ms.load(Ordering::SeqCst)
    }

    fn utc_offset_secs(&self) -> i64 {
        self.offset.load(Ordering::SeqCst)
    }
}

fn parts() -> WorldParts {
    WorldParts::new(
        Scenario::Returning,
        CLOCK,
        HOME.into(),
        format!("{HOME}/src/demo"),
        Box::new(SimOnePassword::fixture(CLOCK.now_secs())),
        SimBackend::shared(),
    )
}

fn wall_world(wall: &Arc<FakeWall>) -> World {
    World::new(parts().wall(wall.clone()))
}

#[test]
fn the_sim_source_advances_by_the_tick_interval() {
    let mut world = World::new(parts());
    let start = world.clock.now_ms;
    world.tick(200);
    world.tick(200);
    assert_eq!(world.clock.now_ms, start + 400);
    assert_eq!(world.now_secs(), CLOCK.now_secs());
}

#[test]
fn the_wall_source_starts_at_the_source_time() {
    let wall = FakeWall::at(NOON_UTC_MS + 250, 3600);
    let world = wall_world(&wall);
    assert_eq!(world.now_secs(), NOON_UTC_MS / 1000);
    assert_eq!(world.last_refresh_secs, NOON_UTC_MS / 1000);
    assert_eq!(world.clock.hhmm(world.now_secs()), "13:00");
}

#[test]
fn the_wall_source_follows_the_source_not_the_interval() {
    let wall = FakeWall::at(NOON_UTC_MS, 0);
    let mut world = wall_world(&wall);
    wall.advance(90_500);
    world.tick(200);
    assert_eq!(world.now_secs(), NOON_UTC_MS / 1000 + 90);
    assert_eq!(world.clock.ago(NOON_UTC_MS / 1000), "1 min ago");
    world.tick(200);
    assert_eq!(world.now_secs(), NOON_UTC_MS / 1000 + 90);
}

#[test]
fn the_wall_source_follows_a_changed_utc_offset() {
    let wall = FakeWall::at(NOON_UTC_MS, 0);
    let mut world = wall_world(&wall);
    assert_eq!(world.clock.hhmm(world.now_secs()), "12:00");
    wall.offset.store(7200, Ordering::SeqCst);
    world.tick(200);
    assert_eq!(world.clock.hhmm(world.now_secs()), "14:00");
}

#[test]
fn a_paused_clock_does_not_follow_the_wall() {
    let wall = FakeWall::at(NOON_UTC_MS, 0);
    let mut world = wall_world(&wall);
    world.clock.running = false;
    wall.advance(60_000);
    world.tick(200);
    assert_eq!(world.now_secs(), NOON_UTC_MS / 1000);
    world.clock.running = true;
    world.tick(200);
    assert_eq!(world.now_secs(), NOON_UTC_MS / 1000 + 60);
}
