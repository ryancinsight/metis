use super::*;
use crate::style::{Color, LinearGradient};
use metis_platform::framebuffer::{Damage, Framebuffer};
use metis_platform::rasterizer::{BoxShadow, CornerRadius, LineCap, LineJoin, StrokeWidth};
use metis_platform::typeface::{GlyphWeight, TextSize, TextStyle};

/// Deterministic xorshift source so a failing edit replays exactly.
struct Sequence(u64);

impl Sequence {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn in_range(&mut self, low: i32, high: i32) -> i32 {
        let span = u64::from(high.abs_diff(low)) + 1;
        low + i32::try_from(self.next() % span).expect("bounded span fits i32")
    }

    fn color(&mut self) -> Color {
        let bits = self.next();
        let channel = |shift: u32| u8::try_from((bits >> shift) & 0xff).expect("one byte");
        Color::rgba(channel(0), channel(8), channel(16), channel(24).max(32))
    }

    fn rect(&mut self) -> Rect {
        Rect::new(
            self.in_range(-15, 90),
            self.in_range(-15, 70),
            self.in_range(1, 50),
            self.in_range(1, 40),
        )
    }

    /// One command of a kind chosen by `kind`.
    fn command(&mut self, kind: u64) -> DisplayCommand {
        let rect = self.rect();
        let radius = CornerRadius::clamped(self.in_range(0, 10), rect);
        match kind % 8 {
            0 => DisplayCommand::ElementRect {
                id: format!("element-{}", self.in_range(0, 3)),
                rect,
            },
            1 => DisplayCommand::DrawShadow {
                rect,
                radius,
                shadow: BoxShadow::new(
                    self.in_range(-5, 5),
                    self.in_range(-5, 5),
                    u32::try_from(self.in_range(0, 14)).expect("nonnegative blur"),
                    self.color(),
                )
                .expect("blur within range"),
            },
            2 => DisplayCommand::FillRect {
                rect,
                radius,
                color: self.color(),
            },
            3 => {
                let stops = [
                    metis_platform::rasterizer::GradientStop {
                        color: self.color(),
                        position: None,
                    },
                    metis_platform::rasterizer::GradientStop {
                        color: self.color(),
                        position: None,
                    },
                ];
                DisplayCommand::FillGradient {
                    rect,
                    radius,
                    gradient: LinearGradient::new(f64::from(self.in_range(0, 359)), &stops)
                        .expect("two finite stops"),
                }
            }
            4 => DisplayCommand::DrawBorder {
                rect,
                width: self.in_range(1, 3),
                radius,
                color: self.color(),
            },
            5 => DisplayCommand::DrawLine {
                start: (self.in_range(-20, 110), self.in_range(-20, 90)),
                end: (self.in_range(-20, 110), self.in_range(-20, 90)),
                color: self.color(),
            },
            6 => DisplayCommand::DrawPolyline {
                points: vec![
                    (self.in_range(-10, 100), self.in_range(-10, 80)),
                    (self.in_range(-10, 100), self.in_range(-10, 80)),
                ],
                width: StrokeWidth::new(u32::try_from(self.in_range(1, 5)).expect("positive"))
                    .expect("width within range"),
                cap: LineCap::Round,
                join: LineJoin::Round,
                color: self.color(),
            },
            _ => DisplayCommand::DrawText {
                text: ["Ag", "Wy", "Metis", "jq|"]
                    [usize::try_from(self.next() % 4).expect("index")]
                .to_owned(),
                x: self.in_range(-10, 80),
                y: self.in_range(-10, 60),
                style: TextStyle::new(
                    self.color(),
                    TextSize::new(f64::from(self.in_range(8, 22))).expect("size within range"),
                )
                .with_weight(GlyphWeight::Bold),
            },
        }
    }
}

const WIDTH: u32 = 96;
const HEIGHT: u32 = 72;
const BACKDROP: Color = Color::rgba(240, 244, 248, 255);

fn paint(list: &DisplayList) -> Framebuffer {
    let mut fb = Framebuffer::new(WIDTH, HEIGHT).expect("surface");
    fb.clear(BACKDROP);
    list.render_to(&mut fb);
    fb
}

#[test]
fn repainting_the_damage_matches_a_full_repaint() {
    let mut sequence = Sequence(0x243f_6a88_85a3_08d3);
    let mut regions = 0;
    for case in 0..160 {
        let length = sequence.in_range(1, 9);
        let painted = DisplayList {
            commands: (0..length)
                .map(|_| {
                    let kind = sequence.next();
                    sequence.command(kind)
                })
                .collect(),
        };
        let mut next = painted.clone();
        // Replace one or two commands, keeping or changing their kinds.
        for _ in 0..sequence.in_range(1, 2) {
            let slot =
                usize::try_from(sequence.next() % next.commands.len() as u64).expect("slot index");
            let kind = sequence.next();
            next.commands[slot] = sequence.command(kind);
        }
        let mut surface = paint(&painted);
        let whole = Rect::new(
            0,
            0,
            i32::try_from(WIDTH).expect("width fits"),
            i32::try_from(HEIGHT).expect("height fits"),
        );
        match next.damage_since(&painted, whole) {
            Damage::Unchanged => {}
            Damage::Region(region) => {
                regions += 1;
                surface.render_clipped(region, |fb| {
                    fb.clear(BACKDROP);
                    next.render_to(fb);
                });
            }
            Damage::Full => {
                surface.clear(BACKDROP);
                next.render_to(&mut surface);
            }
        }
        assert_eq!(
            surface.pixels(),
            paint(&next).pixels(),
            "case {case}: the damaged repaint diverged from a full repaint"
        );
    }
    assert!(
        regions > 80,
        "the cases exercise damaged regions: {regions}"
    );
}

#[test]
fn metadata_only_changes_and_length_changes_are_classified() {
    let element = |id: &str| DisplayCommand::ElementRect {
        id: id.to_owned(),
        rect: Rect::new(0, 0, 4, 4),
    };
    let painted = DisplayList {
        commands: vec![element("a")],
    };
    let renamed = DisplayList {
        commands: vec![element("b")],
    };
    let surface = Rect::new(0, 0, 8, 8);
    assert_eq!(renamed.damage_since(&painted, surface), Damage::Unchanged);
    let longer = DisplayList {
        commands: vec![element("a"), element("b")],
    };
    assert_eq!(longer.damage_since(&painted, surface), Damage::Full);
}
