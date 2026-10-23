use std::io::{self, Write};

pub type Rgb = [u8; 3];

pub fn lerp(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    [0, 1, 2].map(|i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t).round() as u8)
}

/// Pixel canvas. Every terminal cell holds two pixels (top and bottom half block).
pub struct Frame {
    pub w: usize,
    pub h: usize,
    px: Vec<Rgb>,
}

impl Frame {
    pub fn new(w: usize, h: usize) -> Self {
        Self { w, h, px: vec![[0, 0, 0]; w * h] }
    }

    pub fn set(&mut self, x: i32, y: i32, c: Rgb) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
            self.px[y as usize * self.w + x as usize] = c;
        }
    }

    pub fn blend(&mut self, x: i32, y: i32, c: Rgb, a: f32) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
            let i = y as usize * self.w + x as usize;
            self.px[i] = lerp(self.px[i], c, a);
        }
    }

    /// Blends a size x size block with its top-left corner at (x, y).
    pub fn dot(&mut self, x: i32, y: i32, size: i32, c: Rgb, a: f32) {
        for dy in 0..size {
            for dx in 0..size {
                self.blend(x + dx, y + dy, c, a);
            }
        }
    }

    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Rgb) {
        let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
        let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
        let (mut x, mut y, mut err) = (x0, y0, dx + dy);
        loop {
            self.set(x, y, c);
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    pub fn at(&self, x: usize, y: usize) -> Rgb {
        self.px[y * self.w + x]
    }

    pub fn row_mut(&mut self, y: usize) -> &mut [Rgb] {
        &mut self.px[y * self.w..(y + 1) * self.w]
    }
}

/// Text drawn over the pixels (HUD). One entry per terminal cell.
pub struct Overlay {
    w: usize,
    rows: usize,
    cells: Vec<Option<(char, Rgb, u8)>>,
}

impl Overlay {
    pub fn new(w: usize, rows: usize) -> Self {
        Self { w, rows, cells: vec![None; w * rows] }
    }

    pub fn text(&mut self, col: usize, row: usize, s: &str, fg: Rgb) {
        if row >= self.rows {
            return;
        }
        for (i, ch) in s.chars().enumerate() {
            if col + i < self.w {
                self.cells[row * self.w + col + i] = Some((ch, fg, 28));
            }
        }
    }

    pub fn emphasized_text(&mut self, col: usize, row: usize, s: &str, fg: Rgb) {
        if row >= self.rows {
            return;
        }
        for (i, ch) in s.chars().enumerate() {
            if col + i < self.w {
                self.cells[row * self.w + col + i] = Some((ch, fg, 70));
            }
        }
    }

    fn get(&self, col: usize, row: usize) -> Option<(char, Rgb, u8)> {
        self.cells[row * self.w + col]
    }
}

#[derive(Clone, Copy, PartialEq)]
struct Cell {
    ch: char,
    fg: Rgb,
    bg: Rgb,
}

/// Sextant glyph (2 columns x 3 rows of sub-pixels) for a mask whose bit 0 is the upper-left
/// sub-pixel, then upper-right, middle-left, middle-right, lower-left, lower-right.
fn sextant(mask: u8) -> char {
    match mask {
        0 => ' ',
        63 => '█',
        21 => '▌',
        42 => '▐',
        m => {
            let m = m as u32;
            let skip = (m > 21) as u32 + (m > 42) as u32;
            char::from_u32(0x1FB00 + (m - 1) - skip).unwrap_or('█')
        }
    }
}

fn dist2(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}

/// Splits six sub-pixel colours into two groups (2-means). Returns the mask of the light group and both colours.
fn split_two(px: &[[f32; 3]; 6]) -> (u8, Rgb, Rgb) {
    let lum = |c: &[f32; 3]| 0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2];
    let (mut lo, mut hi) = (0, 0);
    for i in 0..6 {
        if lum(&px[i]) < lum(&px[lo]) {
            lo = i;
        }
        if lum(&px[i]) > lum(&px[hi]) {
            hi = i;
        }
    }
    let (mut ca, mut cb) = (px[lo], px[hi]);
    let mut mask = 0u8;
    for _ in 0..2 {
        mask = 0;
        let (mut sa, mut na, mut sb, mut nb) = ([0.0; 3], 0.0, [0.0; 3], 0.0);
        for (i, p) in px.iter().enumerate() {
            if dist2(*p, cb) < dist2(*p, ca) {
                mask |= 1 << i;
                for k in 0..3 { sb[k] += p[k]; }
                nb += 1.0;
            } else {
                for k in 0..3 { sa[k] += p[k]; }
                na += 1.0;
            }
        }
        if na > 0.0 { ca = [sa[0] / na, sa[1] / na, sa[2] / na]; }
        if nb > 0.0 { cb = [sb[0] / nb, sb[1] / nb, sb[2] / nb]; }
    }
    let q = |c: [f32; 3]| [c[0].round() as u8, c[1].round() as u8, c[2].round() as u8];
    (mask, q(cb), q(ca))
}

/// Diffing renderer. The frame is 2 pixels wide and 4 pixels tall per terminal cell (square
/// pixels); each cell is resampled to a 2x3 sextant and coloured with its best two colours,
/// so detail is finer than the cell grid. Only cells that changed are written.
pub struct Presenter {
    w: usize,
    rows: usize,
    prev: Vec<Cell>,
    /// The 2x4 source pixels and overlay cell each cell was last computed from: a cell whose
    /// inputs did not change is skipped before any colour fitting.
    src: Vec<[Rgb; 8]>,
    ov: Vec<Option<(char, Rgb, u8)>>,
}

impl Presenter {
    pub fn new(w: usize, rows: usize) -> Self {
        let blank = Cell { ch: '\0', fg: [0; 3], bg: [0; 3] };
        Self { w, rows, prev: vec![blank; w * rows], src: vec![[[0; 3]; 8]; w * rows], ov: vec![None; w * rows] }
    }

    pub fn invalidate(&mut self) {
        for c in &mut self.prev {
            c.ch = '\0';
        }
    }

    pub fn present<W: Write>(&mut self, f: &Frame, o: &Overlay, out: &mut W) -> io::Result<()> {
        out.write_all(b"\x1b[?2026h")?;
        let mut cursor: Option<(usize, usize)> = None;
        let mut fg: Option<Rgb> = None;
        let mut bg: Option<Rgb> = None;
        let mut utf8 = [0u8; 4];
        // Vertical weights mapping 4 frame rows onto 3 sub-rows.
        const WY: [[f32; 4]; 3] = [[1.0, 1.0 / 3.0, 0.0, 0.0], [0.0, 2.0 / 3.0, 2.0 / 3.0, 0.0], [0.0, 0.0, 1.0 / 3.0, 1.0]];
        for r in 0..self.rows {
            for c in 0..self.w {
                let i = r * self.w + c;
                let mut src = [[0u8; 3]; 8];
                for k in 0..4 {
                    src[k * 2] = f.at(c * 2, r * 4 + k);
                    src[k * 2 + 1] = f.at(c * 2 + 1, r * 4 + k);
                }
                let ov = o.get(c, r);
                if self.prev[i].ch != '\0' && self.src[i] == src && self.ov[i] == ov {
                    continue;
                }
                self.src[i] = src;
                self.ov[i] = ov;
                let mut px = [[0.0f32; 3]; 6];
                let mut avg = [0.0f32; 3];
                for j in 0..3 {
                    let norm: f32 = WY[j].iter().sum();
                    for i in 0..2 {
                        let mut acc = [0.0f32; 3];
                        for (k, wgt) in WY[j].iter().enumerate() {
                            if *wgt > 0.0 {
                                let p = f.at(c * 2 + i, r * 4 + k);
                                for ch in 0..3 { acc[ch] += p[ch] as f32 * wgt; }
                            }
                        }
                        for ch in 0..3 {
                            acc[ch] /= norm;
                            avg[ch] += acc[ch] / 6.0;
                        }
                        px[j * 2 + i] = acc;
                    }
                }
                let cell = match o.get(c, r) {
                    // Dark plate behind text keeps the HUD readable on bright water.
                    Some((ch, col, plate)) => Cell {
                        ch,
                        fg: col,
                        bg: lerp([avg[0] as u8, avg[1] as u8, avg[2] as u8], [4, 22, 38], plate as f32 / 100.0),
                    },
                    None => {
                        let (mask, hi, lo) = split_two(&px);
                        if mask == 0 || mask == 63 {
                            let a = [avg[0].round() as u8, avg[1].round() as u8, avg[2].round() as u8];
                            Cell { ch: ' ', fg: a, bg: a }
                        } else {
                            Cell { ch: sextant(mask), fg: hi, bg: lo }
                        }
                    }
                };
                if self.prev[i] == cell {
                    continue;
                }
                self.prev[i] = cell;
                if cursor != Some((r, c)) {
                    write!(out, "\x1b[{};{}H", r + 1, c + 1)?;
                }
                if fg != Some(cell.fg) {
                    write!(out, "\x1b[38;2;{};{};{}m", cell.fg[0], cell.fg[1], cell.fg[2])?;
                    fg = Some(cell.fg);
                }
                if bg != Some(cell.bg) {
                    write!(out, "\x1b[48;2;{};{};{}m", cell.bg[0], cell.bg[1], cell.bg[2])?;
                    bg = Some(cell.bg);
                }
                out.write_all(cell.ch.encode_utf8(&mut utf8).as_bytes())?;
                cursor = Some((r, c + 1));
            }
        }
        out.write_all(b"\x1b[0m\x1b[?2026l")?;
        out.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sextant_masks_map_to_distinct_glyphs() {
        let mut seen = std::collections::HashSet::new();
        for m in 0..=63u8 {
            assert!(seen.insert(sextant(m)), "mask {m} collides");
        }
        assert_eq!(sextant(0), ' ');
        assert_eq!(sextant(63), '█');
        assert_eq!(sextant(21), '▌');
        assert_eq!(sextant(42), '▐');
        assert_eq!(sextant(1), '\u{1FB00}', "upper-left sub-pixel only");
        assert_eq!(sextant(62), '\u{1FB3B}', "everything but the upper-left");
    }

    #[test]
    fn split_two_separates_light_from_dark() {
        let (dark, light) = ([10.0; 3], [200.0; 3]);
        // upper-left and middle-right are light
        let px = [light, dark, dark, light, dark, dark];
        let (mask, hi, lo) = split_two(&px);
        assert_eq!(mask, 0b001001);
        assert_eq!((hi, lo), ([200; 3], [10; 3]));
    }

    fn present(f: &Frame, o: &Overlay) -> String {
        let mut out = Vec::new();
        Presenter::new(f.w / 2, f.h / 4).present(f, o, &mut out).unwrap();
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn flat_colour_becomes_blank_cells_and_text_gets_a_plate() {
        let mut f = Frame::new(4, 8);
        for y in 0..8 {
            for x in 0..4 {
                f.set(x, y, [20, 40, 80]);
            }
        }
        let flat = present(&f, &Overlay::new(2, 2));
        assert!(!flat.contains('█') && flat.contains(' '));
        let mut o = Overlay::new(2, 2);
        o.text(0, 0, "A", [255, 255, 255]);
        let with_text = present(&f, &o);
        assert!(with_text.contains('A'));
        assert!(with_text.contains("48;2;16;35;68"), "plate lightly blends the local water colour with a blue tint");
    }

    #[test]
    fn emphasized_text_uses_a_darker_translucent_plate() {
        let mut f = Frame::new(4, 8);
        for y in 0..8 {
            for x in 0..4 {
                f.set(x, y, [200, 220, 230]);
            }
        }
        let mut o = Overlay::new(2, 2);
        o.text(0, 0, "N", [255, 255, 255]);
        o.emphasized_text(1, 0, "→", [255, 255, 255]);
        let rendered = present(&f, &o);
        assert!(rendered.contains("48;2;145;165;176"), "ordinary text keeps the light plate");
        assert!(rendered.contains("48;2;63;81;96"), "emphasized text darkens the plate");
    }

    #[test]
    fn emphasized_plate_stays_dark_over_white_water() {
        let mut f = Frame::new(4, 8);
        for y in 0..8 {
            for x in 0..4 {
                f.set(x, y, [255, 255, 255]);
            }
        }
        let mut o = Overlay::new(2, 2);
        o.emphasized_text(0, 0, "→", [140, 230, 246]);
        assert!(present(&f, &o).contains("48;2;79;92;103"));
    }

    #[test]
    fn only_changed_cells_are_rewritten() {
        let mut f = Frame::new(4, 8);
        let mut p = Presenter::new(2, 2);
        let o = Overlay::new(2, 2);
        let mut first = Vec::new();
        p.present(&f, &o, &mut first).unwrap();
        let mut again = Vec::new();
        p.present(&f, &o, &mut again).unwrap();
        assert!(again.len() < first.len() / 2, "an unchanged frame writes (almost) nothing");
        f.set(0, 0, [255, 0, 0]);
        let mut changed = Vec::new();
        p.present(&f, &o, &mut changed).unwrap();
        assert!(String::from_utf8(changed).unwrap().contains("\x1b[1;1H"));
    }
}
