use super::*;
#[test]
fn cancelled_operations_drop_late_join_and_paste_results_before_next_panel()
-> Result<(), AssetError> {
    let (tx, receiver) = mpsc::sync_channel(4);
    tx.send(LocalUpdate::Joined(Ok(false)))?;
    tx.send(LocalUpdate::AddressText(Ok("old address".into())))?;
    tx.send(LocalUpdate::Done(Ok(())))?;
    let mut worker = LocalWorker {
        active: Some(Active {
            #[cfg(windows)]
            native_picker: false,
            cancel: Arc::new(AtomicBool::new(true)),
            receiver,
            handle: std::thread::spawn(|| {}),
            importing: false,
            mouse_game: false,
            mutating: false,
        }),
        operations: 1,
    };
    let updates = worker.poll();
    assert_eq!(updates.len(), 1);
    assert!(matches!(updates[0], LocalUpdate::Done(Ok(()))));
    assert!(!worker.active());
    Ok(())
}

#[test]
fn uncancelled_operations_deliver_current_join_and_paste_receipts_before_done()
-> Result<(), AssetError> {
    let (tx, receiver) = mpsc::sync_channel(4);
    tx.send(LocalUpdate::Joined(Ok(true)))?;
    tx.send(LocalUpdate::AddressText(Ok("current address".into())))?;
    tx.send(LocalUpdate::Done(Ok(())))?;
    let mut worker = LocalWorker {
        active: Some(Active {
            #[cfg(windows)]
            native_picker: false,
            cancel: Arc::new(AtomicBool::new(false)),
            receiver,
            handle: std::thread::spawn(|| {}),
            importing: false,
            mouse_game: false,
            mutating: false,
        }),
        operations: 1,
    };
    let updates = worker.poll();
    assert_eq!(updates.len(), 3);
    assert!(matches!(updates[0], LocalUpdate::Joined(Ok(true))));
    assert!(matches!(&updates[1], LocalUpdate::AddressText(Ok(text)) if text == "current address"));
    assert!(matches!(updates[2], LocalUpdate::Done(Ok(()))));
    assert!(!worker.active());
    assert!(worker.poll().is_empty());
    Ok(())
}

#[cfg(windows)]
#[test]
fn cancelled_native_picker_drops_chosen_path_before_it_can_start_a_write() -> Result<(), AssetError>
{
    let (tx, receiver) = mpsc::sync_channel(2);
    tx.send(LocalUpdate::Picked(
        Action::SaveAs,
        Ok(vec![PathBuf::from("C:\\test.tack")]),
    ))?;
    tx.send(LocalUpdate::Done(Ok(())))?;
    let mut worker = LocalWorker {
        active: Some(Active {
            native_picker: true,
            cancel: Arc::new(AtomicBool::new(true)),
            receiver,
            handle: std::thread::spawn(|| {}),
            importing: false,
            mouse_game: false,
            mutating: false,
        }),
        operations: 1,
    };
    let updates = worker.poll();
    assert_eq!(updates.len(), 1);
    assert!(matches!(updates[0], LocalUpdate::Done(Ok(()))));
    assert!(!worker.native_picker_active());
    Ok(())
}
