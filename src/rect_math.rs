pub struct Vec2 {
    pub x: i32,
    pub y: i32,
}

pub struct Rect {
    pub pos: Vec2,
    pub size: Vec2,
}

/// Ensure rectangle `a` is inside rectangle `b`, with padding `pad`.
/// Returns a rectangle that is ensured to be inside `b`, by moving it if necessary.
pub fn rect_ensure_within(a: Rect, b: Rect, pad: Vec2) -> Rect {
    let mut new = a;

    // Calculate the boundaries of the inner rectangle
    let inner_left = b.pos.x + pad.x;
    let inner_right = b.pos.x + b.size.x - pad.x;
    let inner_top = b.pos.y + pad.y;
    let inner_bottom = b.pos.y + b.size.y - pad.y;

    // Calculate the boundaries of the outer rectangle
    let outer_left = new.pos.x;
    let outer_right = new.pos.x + new.size.x;
    let outer_top = new.pos.y;
    let outer_bottom = new.pos.y + new.size.y;

    // Adjust the position of the inner rectangle if necessary
    if outer_left < inner_left {
        new.pos.x += inner_left - outer_left;
    } else if outer_right > inner_right {
        new.pos.x -= outer_right - inner_right;
    }

    if outer_top < inner_top {
        new.pos.y += inner_top - outer_top;
    } else if outer_bottom > inner_bottom {
        new.pos.y -= outer_bottom - inner_bottom;
    }

    new
}
