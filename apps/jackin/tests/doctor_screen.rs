#![cfg(feature = "demo")]

mod common;

use common::*;
use jackin_console::domain::doctor::DoctorState;
use jackin_console::fixtures::{Scenario, world_for};

#[test]
fn the_first_run_in_progress_says_so_and_offers_no_second_run() {
    let mut world = world_for(Scenario::Doctor);
    world.surfaces.doctor.as_mut().expect("doctor").state = DoctorState::Running { previous: None };
    let mut app = app_with(Scenario::Doctor, world);
    press(&mut app, "D");

    let shown = screen(&mut app);
    assert!(shown.contains("Running checks…"), "{shown}");
    assert!(!shown.contains("Checks have not run yet"), "{shown}");
    assert!(!shown.contains("Run checks"), "{shown}");
}

#[test]
fn a_doctor_that_never_ran_offers_the_first_run() {
    let mut world = world_for(Scenario::Doctor);
    world.surfaces.doctor.as_mut().expect("doctor").state = DoctorState::NeverRun;
    let mut app = app_with(Scenario::Doctor, world);
    press(&mut app, "D");

    let shown = screen(&mut app);
    assert!(shown.contains("Checks have not run yet"), "{shown}");
    assert!(shown.contains("Run checks"), "{shown}");
}
