use tack_core::*;
#[test]
fn shared_commands_wait_for_authority_and_offline_refuses() -> Result<(), Box<dyn std::error::Error>>
{
    let doc = Document::new(DocumentId::new(1)?, DocumentLimits::default());
    let mut local = DocumentEditor::new(doc.clone(), 200);
    let mut shared = DocumentEditor::shared(doc);
    let object = DocumentObject::frame(
        ObjectId::new(2)?,
        "Frame".into(),
        Transform::new([20., 20.], [100., 60.], 0., [false; 2])?,
    )?;
    let command = Command::AddObject { object, index: 0 };
    assert!(shared.execute(command.clone()).is_err());
    shared.set_shared_writable(true);
    assert!(shared.execute(command.clone())?);
    assert_eq!(shared.document().objects().count(), 0);
    let Some(BackendRequest::Edit(request)) = shared.take_backend_request() else {
        return Err("missing semantic request".into());
    };
    assert!(shared.accept_authoritative(request)?);
    assert_eq!(shared.document().objects().count(), 1);
    assert!(!shared.is_dirty());
    assert_eq!(shared.undo_len(), 0);
    assert!(shared.undo()?);
    assert!(matches!(
        shared.take_backend_request(),
        Some(BackendRequest::Undo)
    ));
    assert!(local.execute(command)?);
    assert!(local.is_dirty());
    assert!(local.undo()?);
    assert_eq!(local.document().objects().count(), 0);
    assert!(local.take_backend_request().is_none());
    shared.set_shared_writable(false);
    assert!(shared.redo().is_err());
    Ok(())
}
#[test]
fn shared_queue_is_bounded_and_disconnection_discards_unsent_requests()
-> Result<(), Box<dyn std::error::Error>> {
    let mut editor = DocumentEditor::shared(Document::new(
        DocumentId::new(1)?,
        DocumentLimits::default(),
    ));
    editor.set_shared_writable(true);
    for _ in 0..32 {
        assert!(editor.undo()?);
    }
    assert!(editor.undo().is_err());
    editor.set_shared_writable(false);
    assert!(editor.take_backend_request().is_none());
    assert_eq!(editor.document().objects().count(), 0);
    Ok(())
}
