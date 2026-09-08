use inton_core::{
    engine::Parameters,
    mts::{Master, MasterApi, MasterStatus},
    runtime::{ParameterMailbox, Shared},
};
use std::sync::{Arc, Mutex};
#[derive(Default)]
struct Mock {
    busy: bool,
    registers: usize,
    releases: usize,
    publishes: usize,
    ipc: bool,
    resets: usize,
}
struct Api(Arc<Mutex<Mock>>);
impl MasterApi for Api {
    fn available(&self) -> bool {
        true
    }
    fn can_register(&self) -> bool {
        !self.0.lock().unwrap().busy
    }
    fn register(&mut self) {
        let mut m = self.0.lock().unwrap();
        m.busy = true;
        m.registers += 1;
    }
    fn deregister(&mut self) {
        let mut m = self.0.lock().unwrap();
        m.busy = false;
        m.releases += 1;
    }
    fn publish(&mut self, _: &[f64; 128], _: &str, _: f64) {
        self.0.lock().unwrap().publishes += 1;
    }
    fn supports_recovery(&self) -> bool {
        self.0.lock().unwrap().ipc
    }
    fn reinitialize(&mut self) {
        let mut m = self.0.lock().unwrap();
        m.busy = false;
        m.resets += 1;
    }
    fn clients(&self) -> usize {
        2
    }
}
#[test]
fn master_collision_disable_and_cleanup() {
    let state = Arc::new(Mutex::new(Mock {
        busy: true,
        ..Default::default()
    }));
    let mut master = Master::new(Api(state.clone()));
    assert_eq!(
        master.update(true, &[440.; 128], "a", 2.),
        MasterStatus::BlockedByOtherMaster
    );
    assert_eq!(state.lock().unwrap().registers, 0);
    state.lock().unwrap().busy = false;
    assert_eq!(
        master.update(true, &[440.; 128], "a", 2.),
        MasterStatus::ActiveMaster
    );
    assert_eq!(
        master.update(false, &[440.; 128], "a", 2.),
        MasterStatus::Disabled
    );
    assert_eq!(state.lock().unwrap().releases, 1);
    master.update(true, &[440.; 128], "a", 2.);
    drop(master);
    assert_eq!(state.lock().unwrap().releases, 2);
}
#[test]
fn mailbox_never_tears_a_batch() {
    let mailbox = Arc::new(ParameterMailbox::new());
    let w = mailbox.clone();
    let writer = std::thread::spawn(move || {
        for i in 0..10000 {
            let mut p = Parameters::default();
            if i % 2 == 0 {
                p.position = 7;
                p.reference = 432.;
                p.transpose = 12;
            }
            w.write(p);
        }
    });
    for _ in 0..10000 {
        if let Some(p) = mailbox.read() {
            assert!(
                (p.position == 7 && p.reference == 432. && p.transpose == 12)
                    || (p.position == 0 && p.reference == 440. && p.transpose == 0)
            );
        }
    }
    writer.join().unwrap();
}
#[test]
fn project_change_keeps_other_slots_and_validates_first() {
    let s = Shared::new();
    assert!(s.assign(1, inton_core::tuning::twelve_edo()).is_ok());
    assert!(
        s.assign(
            1,
            inton_core::tuning::Preset::new("bad".into(), "bad".into(), None)
        )
        .is_err()
    );
    assert_eq!(
        s.project.lock().unwrap().project.slots[1]
            .as_ref()
            .unwrap()
            .display_name,
        "12 EDO"
    );
}

#[test]
fn audition_is_temporary_preserves_project_and_uses_current_reference() {
    let shared = inton_core::runtime::Shared::new();
    let before = shared.snapshot().encode().unwrap();
    let lib = inton::library::Library::factory();
    let preset = lib.load("factory:22edo").unwrap();
    let expected = preset.prepare().unwrap();
    shared
        .audition_enabled
        .store(true, std::sync::atomic::Ordering::SeqCst);
    shared.audition_scale(preset).unwrap();
    let (hz, name, period, _) = shared.publication(0.0);
    assert_eq!(name, "22 EDO");
    assert_eq!(period, 2.0);
    assert!((hz[61] - expected.hz[61]).abs() < 1e-8);
    assert_eq!(shared.snapshot().encode().unwrap(), before);
    let mut p = shared.parameters.read().unwrap();
    p.reference = 432.;
    p.transpose = 12;
    shared.parameters.write(p);
    let (hz, _, _, _) = shared.publication(0.0);
    assert!((hz[61] - expected.hz[61] * 432. / 440. * 2.).abs() < 1e-8);
    shared.stop_audition();
    let (_, name, _, _) = shared.publication(0.0);
    assert_eq!(name, "12 EDO");
    assert!(
        shared.project.lock().unwrap().project.slots[0]
            .as_ref()
            .unwrap()
            .display_name
            == "12 EDO"
    );
}

#[test]
fn append_preserves_cleared_slots_and_full_or_invalid_drops_are_atomic() {
    let shared = Shared::new();
    let lib = inton::library::Library::factory();
    let preset = lib.load("factory:22edo").unwrap();
    assert_eq!(shared.append(preset.clone()).unwrap(), 1);
    shared.clear(0).unwrap();
    assert_eq!(shared.append(preset.clone()).unwrap(), 2);
    assert!(shared.snapshot().slots[0].is_none());
    let before = shared.snapshot().encode().unwrap();
    assert!(
        shared
            .append(inton_core::tuning::Preset::new(
                "bad".into(),
                "bad".into(),
                None
            ))
            .is_err()
    );
    assert_eq!(shared.snapshot().encode().unwrap(), before);
    for n in 3..32 {
        assert_eq!(shared.append(preset.clone()).unwrap(), n);
    }
    let before = shared.snapshot().encode().unwrap();
    assert!(shared.append(preset).is_err());
    assert_eq!(shared.snapshot().encode().unwrap(), before);
}

#[test]
fn reorder_preserves_morph_embedded_data_and_numeric_automation() {
    let shared = Shared::new();
    let lib = inton::library::Library::factory();
    shared
        .assign(1, lib.load("factory:22edo").unwrap())
        .unwrap();
    shared
        .assign(3, lib.load("factory:13ed3").unwrap())
        .unwrap();
    let mut p = shared.parameters.read().unwrap();
    p.position = 1;
    p.morph_ms = 2000.;
    shared.parameters.write(p);
    shared.publication(0.);
    let before = shared.publication(0.5).0;
    let progress = shared.project.lock().unwrap().engine.progress();
    shared.move_slot(1, 3).unwrap();
    assert_eq!(shared.parameters.read().unwrap().position, 3);
    assert_eq!(shared.publication(0.).0, before);
    assert_eq!(shared.project.lock().unwrap().engine.progress(), progress);
    assert!(shared.snapshot().slots[1].is_none());
    assert_eq!(
        shared.snapshot().slots[2].as_ref().unwrap().display_name,
        "Bohlen-Pierce · 13 ED3"
    );
    assert_eq!(
        shared.snapshot().slots[3].as_ref().unwrap().display_name,
        "22 EDO"
    );
    shared.publication(0.5);
    assert!((shared.project.lock().unwrap().engine.progress() - 0.5).abs() < 1e-10);
    let saved = shared.snapshot();
    let recalled = inton_core::state::Project::decode(&saved.encode().unwrap()).unwrap();
    assert_eq!(saved.encode().unwrap(), recalled.encode().unwrap());
    shared.move_slot(3, 0).unwrap();
    assert_eq!(shared.parameters.read().unwrap().position, 0);
    shared.move_slot(2, 3).unwrap_err(); // empty source
    let before = shared.snapshot().encode().unwrap();
    assert!(shared.move_slot(0, 32).is_err());
    assert_eq!(shared.snapshot().encode().unwrap(), before);
    let mut p = shared.parameters.read().unwrap();
    p.position = 3;
    p.morph_ms = 0.;
    shared.parameters.write(p);
    assert_eq!(shared.publication(0.).1, "Bohlen-Pierce · 13 ED3");
}

#[test]
fn reorder_remaps_inactive_moves_and_retained_empty_slot_without_a_stale_host_batch() {
    let shared = Shared::new();
    shared
        .assign(
            3,
            inton::library::Library::factory()
                .load("factory:22edo")
                .unwrap(),
        )
        .unwrap();
    let mut p = shared.parameters.read().unwrap();
    p.position = 1;
    shared.parameters.write(p);
    let before = shared.publication(0.).0;
    let (old_batch, stamp) = shared.parameters.read_versioned().unwrap();
    shared.move_slot(3, 0).unwrap();
    assert_eq!(shared.parameters.read().unwrap().position, 2);
    assert!(shared.project.lock().unwrap().engine.empty);
    assert_eq!(shared.publication(0.).0, before);
    assert!(!shared.parameters.try_write_versioned(old_batch, stamp));
    assert_eq!(shared.parameters.read().unwrap().position, 2);
    let saved = shared.snapshot();
    let restored = Shared::new();
    restored
        .restore(inton_core::state::Project::decode(&saved.encode().unwrap()).unwrap())
        .unwrap();
    assert_eq!(restored.publication(0.).0, before);
    shared.move_slot(0, 3).unwrap();
    assert_eq!(shared.parameters.read().unwrap().position, 1);
    assert_eq!(shared.publication(0.).0, before);
}

#[test]
fn stale_ipc_recovery_requires_explicit_request_and_republishes() {
    let state = Arc::new(Mutex::new(Mock {
        busy: true,
        ipc: true,
        ..Default::default()
    }));
    let mut master = Master::new(Api(state.clone()));
    for _ in 0..3 {
        assert_eq!(
            master.update(true, &[440.; 128], "current", 2.),
            MasterStatus::BlockedByOtherMaster
        );
    }
    assert_eq!(state.lock().unwrap().resets, 0);
    assert!(master.recovery_available());
    assert_eq!(
        master.recover_and_update(true, &[440.; 128], "current", 2.),
        MasterStatus::ActiveMaster
    );
    {
        let m = state.lock().unwrap();
        assert_eq!((m.resets, m.registers, m.publishes), (1, 1, 1));
    }
    assert!(!master.recovery_available());
    master.recover_and_update(true, &[440.; 128], "current", 2.);
    assert_eq!(
        state.lock().unwrap().resets,
        1,
        "Never reset a connection we already own"
    );
    drop(master);
    assert_eq!(state.lock().unwrap().releases, 1);
}
#[test]
fn recovery_is_ignored_without_ipc_or_when_disabled_and_validates_first() {
    let state = Arc::new(Mutex::new(Mock {
        busy: true,
        ..Default::default()
    }));
    let mut master = Master::new(Api(state.clone()));
    assert!(!master.recovery_available());
    assert_eq!(
        master.recover_and_update(true, &[440.; 128], "current", 2.),
        MasterStatus::BlockedByOtherMaster
    );
    state.lock().unwrap().ipc = true;
    assert_eq!(
        master.recover_and_update(false, &[440.; 128], "current", 2.),
        MasterStatus::Disabled
    );
    assert_eq!(
        master.recover_and_update(true, &[f64::NAN; 128], "invalid", 2.),
        MasterStatus::Error
    );
    assert_eq!(state.lock().unwrap().resets, 0);
    // If the old owner has released meanwhile, ordinary registration is sufficient.
    state.lock().unwrap().busy = false;
    assert_eq!(
        master.recover_and_update(true, &[440.; 128], "current", 2.),
        MasterStatus::ActiveMaster
    );
    assert_eq!(state.lock().unwrap().resets, 0);
}

#[test]
fn deleting_active_scale_selects_first_remaining_and_preserves_gaps() {
    let shared = Shared::new();
    let lib = inton::library::Library::factory();
    shared
        .assign(2, lib.load("factory:19edo").unwrap())
        .unwrap();
    shared
        .assign(5, lib.load("factory:22edo").unwrap())
        .unwrap();
    shared.parameters.write(Parameters {
        position: 5,
        ..Parameters::default()
    });
    shared.clear(5).unwrap();
    assert_eq!(shared.snapshot().parameters.position, 0);
    shared.clear(0).unwrap();
    assert_eq!(shared.snapshot().parameters.position, 2);
    assert!(shared.snapshot().slots[0].is_none());
    assert_eq!(
        shared.snapshot().slots[2].as_ref().unwrap().display_name,
        "19 EDO"
    );
    let tuning = shared.publication(0.).0;
    shared.clear(2).unwrap();
    assert_eq!(shared.publication(0.).0, tuning);
}

#[test]
fn folded_set_preserves_slots_and_final_delete_returns_to_single() {
    let shared = Shared::new();
    shared.enable_scale_set();
    shared
        .assign(
            2,
            inton::library::Library::factory()
                .load("factory:19edo")
                .unwrap(),
        )
        .unwrap();
    let slots = shared.snapshot().slots;
    shared.fold_scale_set(true);
    let recalled =
        inton_core::state::Project::decode(&shared.snapshot().encode().unwrap()).unwrap();
    assert!(recalled.uses_scale_set() && recalled.set_collapsed);
    assert_eq!(recalled.slots, slots);
    shared.restore(recalled).unwrap();
    shared.clear(0).unwrap();
    shared.clear(2).unwrap();
    let project = shared.snapshot();
    assert!(!project.uses_scale_set());
    assert!(!project.set_collapsed);
    assert_eq!(project.parameters.position, 0);
    assert_eq!(project.slots[0].as_ref().unwrap().display_name, "19 EDO");
}

#[test]
fn clear_set_is_one_undo_step_and_redo_keeps_nonstructural_settings() {
    let shared = Shared::new();
    shared
        .append_scales((1..32).map(|_| inton_core::tuning::twelve_edo()).collect())
        .unwrap();
    let mut parameters = shared.parameters.read().unwrap();
    parameters.position = 17;
    shared.parameters.write(parameters);
    let before = shared.snapshot();
    shared.clear_scale_set().unwrap();
    assert_eq!(shared.snapshot().slots.iter().flatten().count(), 1);
    assert_eq!(shared.snapshot().parameters.position, 0);
    assert!(!shared.snapshot().uses_scale_set());
    let mut parameters = shared.parameters.read().unwrap();
    parameters.morph_ms = 700.;
    shared.parameters.write(parameters);
    shared.undo_edit(false).unwrap();
    let restored = shared.snapshot();
    assert_eq!(restored.slots, before.slots);
    assert_eq!(restored.allocated_slots, before.allocated_slots);
    assert_eq!(restored.parameters.position, 17);
    assert_eq!(restored.parameters.morph_ms, 700.);
    shared.undo_edit(true).unwrap();
    assert_eq!(shared.snapshot().slots.iter().flatten().count(), 1);
    assert_eq!(shared.snapshot().parameters.morph_ms, 700.);
}

#[test]
fn undo_restores_gaps_and_failed_or_new_edits_handle_redo_correctly() {
    let shared = Shared::new();
    shared
        .assign(
            3,
            inton::library::Library::factory()
                .load("factory:19edo")
                .unwrap(),
        )
        .unwrap();
    shared.clear(0).unwrap();
    let before = shared.snapshot();
    shared.move_slot(3, 1).unwrap();
    shared.undo_edit(false).unwrap();
    assert_eq!(shared.snapshot().slots, before.slots);
    assert_eq!(shared.snapshot().allocated_slots, before.allocated_slots);
    let labels = shared.history_labels();
    assert!(shared.assign(99, inton_core::tuning::twelve_edo()).is_err());
    assert_eq!(shared.history_labels(), labels);
    shared.assign(1, inton_core::tuning::twelve_edo()).unwrap();
    assert!(shared.history_labels().1.is_none());
    let saved = shared.snapshot();
    shared.restore(saved).unwrap();
    assert_eq!(shared.history_labels(), (None, None));
}

#[test]
fn batch_addition_is_atomic_and_one_undo_step() {
    let shared = Shared::new();
    let before = shared.snapshot();
    assert!(
        shared
            .append_scales((0..32).map(|_| inton_core::tuning::twelve_edo()).collect())
            .is_err()
    );
    assert_eq!(shared.snapshot().slots, before.slots);
    assert_eq!(shared.history_labels(), (None, None));
    shared
        .append_scales((0..4).map(|_| inton_core::tuning::twelve_edo()).collect())
        .unwrap();
    shared.undo_edit(false).unwrap();
    assert_eq!(shared.snapshot().slots, before.slots);
    shared.undo_edit(true).unwrap();
    assert_eq!(shared.snapshot().slots.iter().flatten().count(), 5);
}

#[test]
fn undo_does_not_roll_back_later_host_position_automation() {
    let shared = Shared::new();
    shared.assign(1, inton_core::tuning::twelve_edo()).unwrap();
    shared
        .assign(
            0,
            inton::library::Library::factory()
                .load("factory:19edo")
                .unwrap(),
        )
        .unwrap();
    let mut parameters = shared.parameters.read().unwrap();
    parameters.position = 1;
    shared.parameters.write(parameters);
    shared.undo_edit(false).unwrap();
    assert_eq!(shared.snapshot().parameters.position, 1);
    assert_eq!(
        shared.snapshot().slots[0].as_ref().unwrap().display_name,
        "12 EDO"
    );
}

#[test]
fn clearing_while_an_empty_slot_holds_tuning_preserves_the_table() {
    let shared = Shared::new();
    shared
        .assign(
            2,
            inton::library::Library::factory()
                .load("factory:19edo")
                .unwrap(),
        )
        .unwrap();
    let mut parameters = shared.parameters.read().unwrap();
    parameters.position = 31;
    shared.parameters.write(parameters);
    let table = shared.publication(0.).0;
    shared.clear_scale_set().unwrap();
    assert_eq!(shared.publication(0.).0, table);
    shared.undo_edit(false).unwrap();
    assert_eq!(shared.snapshot().parameters.position, 31);
    assert_eq!(shared.publication(0.).0, table);
}
