use super::{
    CANVAS_EVENT_CAPACITY, CanvasEvent, CanvasEventError, CanvasEventQueue, CanvasKeyboardEvent,
    MAX_KEY_NAME_BYTES,
};

#[test]
fn queue_preserves_order_and_capacity() {
    let mut queue = CanvasEventQueue::new();
    for _ in 0..CANVAS_EVENT_CAPACITY {
        assert!(queue.push(CanvasEvent::Wheel(super::CanvasWheelEvent {
            delta_x: 0.0,
            delta_y: 1.0,
            delta_z: 0.0,
            unit: super::CanvasWheelUnit::Pixel,
            x: 2.0,
            y: 3.0,
            content_width: 320.5,
            content_height: 180.25,
            modifiers: super::CanvasModifiers::default(),
            trust: super::CanvasEventTrust::Trusted,
        })));
    }
    let events = queue.take().expect("queue capacity is valid");
    assert_eq!(events.len(), CANVAS_EVENT_CAPACITY);
    assert_eq!(events[0], events[CANVAS_EVENT_CAPACITY - 1]);
}

#[test]
fn overflow_discards_stale_events_and_reports_once() {
    let mut queue = CanvasEventQueue::new();
    for _ in 0..=CANVAS_EVENT_CAPACITY {
        queue.push(CanvasEvent::Wheel(super::CanvasWheelEvent {
            delta_x: 0.0,
            delta_y: 1.0,
            delta_z: 0.0,
            unit: super::CanvasWheelUnit::Pixel,
            x: 0.0,
            y: 0.0,
            content_width: 320.5,
            content_height: 180.25,
            modifiers: super::CanvasModifiers::default(),
            trust: super::CanvasEventTrust::Trusted,
        }));
    }
    assert_eq!(queue.take(), Err(CanvasEventError::QueueOverflow));
    assert!(
        queue
            .take()
            .expect("queue recovers after reporting")
            .is_empty()
    );
}

#[test]
fn failure_discards_pending_events_and_recovers() {
    let mut queue = CanvasEventQueue::new();
    queue.push(CanvasEvent::Wheel(super::CanvasWheelEvent {
        delta_x: 0.0,
        delta_y: 1.0,
        delta_z: 0.0,
        unit: super::CanvasWheelUnit::Pixel,
        x: 0.0,
        y: 0.0,
        content_width: 320.5,
        content_height: 180.25,
        modifiers: super::CanvasModifiers::default(),
        trust: super::CanvasEventTrust::Trusted,
    }));
    queue.fail(CanvasEventError::InvalidMetadata);
    assert_eq!(queue.take(), Err(CanvasEventError::InvalidMetadata));
    assert!(
        queue
            .take()
            .expect("queue recovers after failure")
            .is_empty()
    );
}

#[test]
fn queue_preserves_keyboard_metadata_and_phase() {
    let mut queue = CanvasEventQueue::new();
    assert!(
        queue.push(CanvasEvent::Keyboard(
            super::CanvasKeyboardEvent::try_new(
                super::CanvasKeyboardPhase::Down,
                "+".to_owned(),
                "Equal".to_owned(),
                true,
                super::CanvasModifiers::default(),
                super::CanvasEventTrust::Trusted,
            )
            .expect("bounded keyboard metadata is valid"),
        ))
    );
    let events = queue.take().expect("keyboard event is valid");
    assert_eq!(events.len(), 1);
    let CanvasEvent::Keyboard(event) = &events[0] else {
        panic!("queue returned a non-keyboard event");
    };
    assert_eq!(event.phase(), super::CanvasKeyboardPhase::Down);
    assert_eq!(event.key(), "+");
    assert_eq!(event.code(), "Equal");
    assert!(event.is_repeated());
}

#[test]
fn keyboard_metadata_rejects_oversized_names() {
    let key = "x".repeat(MAX_KEY_NAME_BYTES + 1);
    assert_eq!(
        CanvasKeyboardEvent::try_new(
            super::CanvasKeyboardPhase::Down,
            key,
            "KeyX".to_owned(),
            false,
            super::CanvasModifiers::default(),
            super::CanvasEventTrust::Untrusted,
        ),
        Err(CanvasEventError::InvalidMetadata)
    );
}

#[test]
fn canvas_events_preserve_trust_values() {
    let pointer = super::CanvasPointerEvent {
        phase: super::CanvasPointerPhase::Down,
        pointer_id: 1,
        pointer_type: super::CanvasPointerType::Mouse,
        x: 2.25,
        y: 3.75,
        content_width: 320.5,
        content_height: 180.25,
        button: 0,
        buttons: 1,
        modifiers: super::CanvasModifiers::default(),
        primary: true,
        trust: super::CanvasEventTrust::Untrusted,
    };
    assert!(!pointer.is_trusted());
    assert_eq!(pointer.trust(), super::CanvasEventTrust::Untrusted);
    assert_eq!(pointer.x().to_bits(), 2.25f64.to_bits());
    assert_eq!(pointer.y().to_bits(), 3.75f64.to_bits());
    assert_eq!(pointer.content_width().to_bits(), 320.5f64.to_bits());
    assert_eq!(pointer.content_height().to_bits(), 180.25f64.to_bits());

    let wheel = super::CanvasWheelEvent {
        delta_x: 0.0,
        delta_y: 1.0,
        delta_z: 0.0,
        unit: super::CanvasWheelUnit::Pixel,
        x: 2.25,
        y: 3.75,
        content_width: 320.5,
        content_height: 180.25,
        modifiers: super::CanvasModifiers::default(),
        trust: super::CanvasEventTrust::Trusted,
    };
    assert!(wheel.is_trusted());
    assert_eq!(wheel.trust(), super::CanvasEventTrust::Trusted);
    assert_eq!(wheel.x().to_bits(), 2.25f64.to_bits());
    assert_eq!(wheel.y().to_bits(), 3.75f64.to_bits());
    assert_eq!(wheel.content_width().to_bits(), 320.5f64.to_bits());
    assert_eq!(wheel.content_height().to_bits(), 180.25f64.to_bits());

    let keyboard = super::CanvasKeyboardEvent::try_new(
        super::CanvasKeyboardPhase::Down,
        "+".to_owned(),
        "Equal".to_owned(),
        false,
        super::CanvasModifiers::default(),
        super::CanvasEventTrust::Untrusted,
    )
    .expect("bounded keyboard metadata is valid");
    assert!(!keyboard.is_trusted());
    assert_eq!(keyboard.trust(), super::CanvasEventTrust::Untrusted);
}

#[test]
fn canvas_event_exposes_variant_trust() {
    let event = CanvasEvent::Wheel(super::CanvasWheelEvent {
        delta_x: 0.0,
        delta_y: 0.0,
        delta_z: 0.0,
        unit: super::CanvasWheelUnit::Pixel,
        x: 0.0,
        y: 0.0,
        content_width: 320.5,
        content_height: 180.25,
        modifiers: super::CanvasModifiers::default(),
        trust: super::CanvasEventTrust::Untrusted,
    });
    assert!(!event.is_trusted());
    assert_eq!(event.trust(), super::CanvasEventTrust::Untrusted);
}
