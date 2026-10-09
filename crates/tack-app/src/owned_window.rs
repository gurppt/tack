//! Explicit native child windows: bounded waiters, no local-startup worker/polling.
use std::{
    process::Child,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
use tack_assets::AssetError;
static WINDOWS: AtomicUsize = AtomicUsize::new(0);
const MAX_WINDOWS: usize = 16;
pub fn adopt(mut child: Child) -> Result<(), AssetError> {
    if WINDOWS.fetch_add(1, Ordering::AcqRel) >= MAX_WINDOWS {
        WINDOWS.fetch_sub(1, Ordering::AcqRel);
        let _ = child.kill();
        let _ = child.wait();
        return Err("At most 16 native child windows per Tack instance".into());
    }
    let owned = Arc::new(Mutex::new(Some(child)));
    let waiter = Arc::clone(&owned);
    let spawned = std::thread::Builder::new()
        .name("tack-window-close".into())
        .spawn(move || {
            if let Ok(mut slot) = waiter.lock()
                && let Some(mut child) = slot.take()
            {
                let _ = child.wait();
            }
            WINDOWS.fetch_sub(1, Ordering::AcqRel);
        });
    if let Err(e) = spawned {
        if let Ok(mut slot) = owned.lock()
            && let Some(mut child) = slot.take()
        {
            let _ = child.kill();
            let _ = child.wait();
        }
        WINDOWS.fetch_sub(1, Ordering::AcqRel);
        return Err(e.into());
    }
    Ok(())
}
