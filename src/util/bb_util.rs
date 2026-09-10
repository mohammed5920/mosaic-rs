use crate::types::Bb;

///returns the region(s) of `a` NOT covered by `b`.
pub(crate) fn subtract_rect(a: Bb, b: Bb) -> Vec<Bb> {
    let min_x = a.0.0.max(b.0.0);
    let min_y = a.0.1.max(b.0.1);
    let max_x = a.1.0.min(b.1.0);
    let max_y = a.1.1.min(b.1.1);
    let i = if min_x < max_x && min_y < max_y {
        ((min_x, min_y), (max_x, max_y))
    } else {
        return vec![a]
    };

    let mut out = Vec::with_capacity(4);
    let ((ax0, ay0), (ax1, ay1)) = a;
    let ((ix0, iy0), (ix1, iy1)) = i;

    // top strip
    if iy0 > ay0 {
        out.push(((ax0, ay0), (ax1, iy0)));
    }
    // bottom strip
    if iy1 < ay1 {
        out.push(((ax0, iy1), (ax1, ay1)));
    }
    // left strip (only within intersection's y-range)
    if ix0 > ax0 {
        out.push(((ax0, iy0), (ix0, iy1)));
    }
    // right strip
    if ix1 < ax1 {
        out.push(((ix1, iy0), (ax1, iy1)));
    }
    out
}
