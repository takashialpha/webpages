//! `wall`: the shared board, and the frame it is drawn in.

use crate::args::Args;
use crate::shell::{Line, Output, Session, Span, error, pending};
use crate::wall;

/// `wall` reads the board, `wall <x> <y> <char>` writes one cell.
///
/// Both answer later, since both are a request. The entry is in the scrollback
/// by then and the prompt never waits.
pub fn graffiti(_session: &mut Session, args: &Args<'_>) -> Output {
    let write = match *args.operands() {
        [] => None,
        // Coordinates alone clear the cell. The line was split on whitespace,
        // so a space cannot arrive as an argument: leaving it out asks for one.
        [x, y] => Some(format!("{x} {y}")),
        [x, y, cell] => Some(format!("{x} {y} {cell}")),
        [..] => {
            return error("wall: one character at a time. `wall <x> <y> [char]`");
        }
    };

    pending(move |sink| {
        let write = write.clone();
        leptos::task::spawn_local(async move {
            sink.set(match wall::fetch(write.as_deref()).await {
                Ok((200, board)) => framed(&board),
                // The server explains a refusal in a sentence, which is more
                // use than the status code it came with.
                Ok((_, complaint)) => error(format!("wall: {}", complaint.trim())),
                Err(problem) => error(problem),
            });
        });
    })
}

/// The board in a frame, with the axes outside it.
///
/// `0,0` is the bottom left and `y` counts up, like the first quadrant of a
/// graph. The frame is drawn with the box characters the ROM font carries, and
/// its width is where the stylesheet's 85 column measure comes from.
fn framed(board: &str) -> Output {
    // Indexed rather than computed, so there is no cast anywhere.
    const DIGITS: &[u8; 10] = b"0123456789";
    let digit = |n: usize| char::from(DIGITS[n % 10]);

    let gutter = " ".repeat(GUTTER);
    let rule = "─".repeat(wall::COLS);

    let mut lines: Vec<Line> = vec![vec![Span::new(format!("{gutter}┌{rule}┐"), "dim")]];

    // The server sends the top row first, which is the order it is drawn in.
    // Only the number beside each row counts the other way, since the bottom
    // row is row zero.
    lines.extend(
        board
            .lines()
            .take(wall::ROWS)
            .enumerate()
            .map(|(index, row)| {
                vec![
                    Span::new(
                        format!("{:>width$} ", wall::ROWS - 1 - index, width = GUTTER - 1),
                        "dim",
                    ),
                    Span::new("│", "dim"),
                    Span::new(format!("{row:width$}", width = wall::COLS), "accent"),
                    Span::new("│", "dim"),
                ]
            }),
    );

    lines.push(vec![Span::new(format!("{gutter}└{rule}┘"), "dim")]);

    // Indented past the border so a column lines up with the cell above it.
    // Tens below units, so a number reads upwards out of the two rows.
    let axis = format!("{gutter} ");
    let units: String = (0..wall::COLS).map(digit).collect();
    let tens: String = (0..wall::COLS)
        .map(|x| if x % 10 == 0 { digit(x / 10) } else { ' ' })
        .collect();
    lines.push(vec![Span::new(format!("{axis}{units}"), "dim")]);
    lines.push(vec![Span::new(format!("{axis}{tens}"), "dim")]);

    // Kept inside the frame's width: this shares the box that scrolls
    // sideways, so a longer line would make the board scroll to read it.
    lines.push(vec![]);
    lines.push(vec![Span::new(
        format!(
            "{}x{}, 0,0 bottom left. `wall <x> <y> <char>` sets a cell; omit char to clear.",
            wall::COLS,
            wall::ROWS
        ),
        "dim",
    )]);
    Output::Wide(lines)
}

/// The row-number column, including the space after it.
const GUTTER: usize = 3;
