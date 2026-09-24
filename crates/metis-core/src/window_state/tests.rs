use super::{
    MAX_WINDOW_STATE_BYTES, MAX_WINDOW_STATE_COORDINATE, MAX_WINDOW_STATE_DIMENSION, WindowState,
    WindowStateError,
};

#[test]
fn states_are_bounded() {
    assert!(WindowState::new(-1_920, 40, 1_280, 800, true).is_ok());
    let edge = MAX_WINDOW_STATE_COORDINATE;
    assert!(WindowState::new(edge, -edge, 1, MAX_WINDOW_STATE_DIMENSION, false).is_ok());
    for (left, top, width, height) in [
        (edge + 1, 0, 800, 600),
        (0, -edge - 1, 800, 600),
        (i32::MIN, 0, 800, 600),
        (0, 0, 0, 600),
        (0, 0, 800, 0),
        (0, 0, MAX_WINDOW_STATE_DIMENSION + 1, 600),
    ] {
        assert_eq!(
            WindowState::new(left, top, width, height, false),
            Err(WindowStateError::OutOfRange),
            "{left},{top} {width}x{height}"
        );
    }
}

#[test]
fn encoding_round_trips_exactly() {
    let state = WindowState::new(-1_920, 40, 1_280, 800, true).expect("state");
    let text = state.encode();
    assert_eq!(
        text,
        "metis-window-state 1\nleft=-1920\ntop=40\nwidth=1280\nheight=800\nmaximized=1\n"
    );
    assert_eq!(WindowState::decode(&text), Ok(state));
    let restored = WindowState::new(0, 0, 640, 480, false).expect("state");
    assert_eq!(WindowState::decode(&restored.encode()), Ok(restored));
}

#[test]
fn decoding_rejects_every_other_form() {
    let valid = "metis-window-state 1\nleft=10\ntop=20\nwidth=640\nheight=480\nmaximized=0\n";
    let malformed = [
        "",
        valid.trim_end(),
        &valid.replace("state 1", "state 2"),
        &valid.replace("left=10\ntop=20", "top=20\nleft=10"),
        &valid.replace("left=10", "left =10"),
        &valid.replace("left=10", "left=+10"),
        &valid.replace("left=10", "left=010"),
        &valid.replace("left=10", "left=-0"),
        &valid.replace("left=10", "left="),
        &valid.replace("left=10", "left=1e1"),
        &valid.replace("maximized=0", "maximized=true"),
        &valid.replace('\n', "\r\n"),
        &format!("{valid}extra=1\n"),
        &format!("{valid}\n"),
    ];
    for text in malformed {
        assert_eq!(
            WindowState::decode(text),
            Err(WindowStateError::Malformed),
            "{text:?}"
        );
    }
    let huge = valid.replace("width=640", "width=99999999999");
    assert_eq!(
        WindowState::decode(&huge),
        Err(WindowStateError::OutOfRange)
    );
    let wide = valid.replace("width=640", "width=16385");
    assert_eq!(
        WindowState::decode(&wide),
        Err(WindowStateError::OutOfRange)
    );
    let long = format!("{valid}{}", " ".repeat(MAX_WINDOW_STATE_BYTES));
    assert_eq!(WindowState::decode(&long), Err(WindowStateError::TooLarge));
}
