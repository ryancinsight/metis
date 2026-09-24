use super::chord;
use crate::input::{MAX_SHORTCUTS, Modifiers, ShortcutError, ShortcutMap};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    Save,
    Open,
}

#[test]
fn bindings_resolve_exact_modifier_sets() {
    let mut map = ShortcutMap::new();
    map.bind(chord(Modifiers::CTRL, b'S'), Command::Save)
        .expect("bind save");
    map.bind(chord(Modifiers::CTRL, b'O'), Command::Open)
        .expect("bind open");
    assert_eq!(
        map.resolve(chord(Modifiers::CTRL, b'S')),
        Some(&Command::Save)
    );
    assert_eq!(
        map.resolve(chord(Modifiers::CTRL.union(Modifiers::SHIFT), b'S')),
        None
    );
    assert_eq!(
        map.accelerator_of(&Command::Open),
        Some(chord(Modifiers::CTRL, b'O'))
    );
    assert_eq!(map.len(), 2);
}

#[test]
fn conflicts_and_capacity_leave_the_map_unchanged() {
    let mut map = ShortcutMap::new();
    map.bind(chord(Modifiers::CTRL, b'S'), Command::Save)
        .expect("bind save");
    assert_eq!(
        map.bind(chord(Modifiers::CTRL, b'S'), Command::Open),
        Err(ShortcutError::Conflict)
    );
    assert_eq!(
        map.resolve(chord(Modifiers::CTRL, b'S')),
        Some(&Command::Save)
    );

    let mut full = ShortcutMap::new();
    let modifier_sets = (0..16).map(|bits| {
        Modifiers::NONE
            .with(Modifiers::CTRL, bits & 1 != 0)
            .with(Modifiers::ALT, bits & 2 != 0)
            .with(Modifiers::SHIFT, bits & 4 != 0)
            .with(Modifiers::META, bits & 8 != 0)
    });
    let chords = modifier_sets
        .flat_map(|modifiers| (b'A'..=b'Z').map(move |byte| chord(modifiers, byte)))
        .take(MAX_SHORTCUTS + 1)
        .collect::<Vec<_>>();
    for accelerator in &chords[..MAX_SHORTCUTS] {
        full.bind(*accelerator, Command::Save)
            .expect("within bound");
    }
    assert_eq!(
        full.bind(chords[MAX_SHORTCUTS], Command::Open),
        Err(ShortcutError::Full)
    );
    assert_eq!(full.len(), MAX_SHORTCUTS);
}

#[test]
fn unbind_frees_the_accelerator() {
    let mut map = ShortcutMap::new();
    let save = chord(Modifiers::CTRL, b'S');
    map.bind(save, Command::Save).expect("bind save");
    assert_eq!(map.unbind(save), Some(Command::Save));
    assert!(map.is_empty());
    assert_eq!(map.unbind(save), None);
    map.bind(save, Command::Open).expect("rebind");
    assert_eq!(map.iter().collect::<Vec<_>>(), vec![(save, &Command::Open)]);
}
