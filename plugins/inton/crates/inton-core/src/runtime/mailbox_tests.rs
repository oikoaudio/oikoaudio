use super::*;
use crate::engine::Parameter;

#[test]
fn host_updates_survive_a_control_edit_without_waiting() {
    use std::sync::mpsc;
    let shared = Arc::new(Shared::new());
    shared.set_parameter_value(Parameter::Reference, 432.0);
    let (start_tx, start_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let writer = shared.clone();
    let host = std::thread::spawn(move || {
        start_rx.recv().unwrap();
        writer.set_parameter_value(Parameter::Reference, 442.0);
        writer.set_parameter_value(Parameter::Transpose, 7.0);
        writer.set_parameter_value(Parameter::Reference, 444.0);
        done_tx.send(()).unwrap();
    });
    let mut completed_while_held = false;
    shared.parameters.modify(|mut parameters| {
        assert_eq!(parameters.reference, 432.0);
        start_tx.send(()).unwrap();
        completed_while_held = done_rx.recv_timeout(Duration::from_secs(1)).is_ok();
        parameters.position = 2;
        parameters
    });
    // Release the writer before asserting so a regression cannot strand the host thread.
    host.join().unwrap();
    assert!(
        completed_while_held,
        "Host callbacks waited for the control writer"
    );
    let parameters = shared.parameters.read().unwrap();
    assert_eq!(parameters.reference, 444.0);
    assert_eq!(parameters.transpose, 7);
    assert_eq!(parameters.position, 2);
    shared.parameters.modify(|parameters| parameters);
    assert_eq!(shared.parameters.read(), Some(parameters));
}

#[test]
fn pending_edits_coalesce_and_drain_once() {
    let shared = Shared::new();
    shared.edit_parameter(Parameter::Reference, 432.0);
    shared.edit_parameter(Parameter::Reference, 442.0);
    shared.edit_parameter(Parameter::Transpose, -3.0);
    let edits: Vec<_> = shared.take_parameter_edits().collect();
    assert_eq!(
        edits,
        vec![(Parameter::Reference, 442.0), (Parameter::Transpose, -3.0)]
    );
    assert!(shared.take_parameter_edits().next().is_none());
}

#[test]
fn edits_published_during_a_drain_remain_pending() {
    let shared = Shared::new();
    shared.edit_parameter(Parameter::Position, 2.0);
    let mut edits = shared.take_parameter_edits();
    assert_eq!(edits.next(), Some((Parameter::Position, 2.0)));
    shared.edit_parameter(Parameter::Position, 4.0);
    shared.edit_parameter(Parameter::Reference, 432.0);
    assert!(edits.next().is_none());
    assert_eq!(
        shared.take_parameter_edits().collect::<Vec<_>>(),
        vec![(Parameter::Position, 4.0), (Parameter::Reference, 432.0)]
    );
}

#[test]
fn restored_project_discards_queued_editor_edits() {
    let shared = Shared::new();
    shared.edit_parameter(Parameter::Reference, 432.0);
    let mut project = Project::default();
    project.parameters.reference = 444.0;
    shared.restore(project).unwrap();
    assert!(shared.take_parameter_edits().next().is_none());
    assert_eq!(shared.parameters.read().unwrap().reference, 444.0);
}
