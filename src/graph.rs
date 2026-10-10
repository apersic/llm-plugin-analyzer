use crate::domain::{Board, Lane, Node, NodeKey};
use crate::theme;
use eframe::egui::Color32;
use image::{Rgba, RgbaImage};
use std::collections::BTreeMap;
use std::f32::consts::TAU;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct Placed {
    pub key: NodeKey,
    pub label: String,
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub fill: Color32,
    pub ring: Color32,
}

#[derive(Clone, Debug, Default)]
pub struct Layout {
    pub nodes: Vec<Placed>,
    pub edges: Vec<(usize, usize)>,
}

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub pan_x: f32,
    pub pan_y: f32,
    pub zoom: f32,
}

impl Camera {
    pub fn fitted(layout: &Layout, w: f32, h: f32) -> Self {
        if layout.nodes.is_empty() {
            return Self {
                pan_x: w * 0.5,
                pan_y: h * 0.5,
                zoom: 1.0,
            };
        }
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;
        for n in &layout.nodes {
            min_x = min_x.min(n.x - n.r);
            min_y = min_y.min(n.y - n.r);
            max_x = max_x.max(n.x + n.r);
            max_y = max_y.max(n.y + n.r);
        }
        let gw = (max_x - min_x).max(40.0);
        let gh = (max_y - min_y).max(40.0);
        let zoom = (w / gw).min(h / gh) * 0.82;
        Self {
            pan_x: w * 0.5 - (min_x + max_x) * 0.5 * zoom,
            pan_y: h * 0.5 - (min_y + max_y) * 0.5 * zoom,
            zoom,
        }
    }

    pub fn to_screen(&self, x: f32, y: f32) -> (f32, f32) {
        (x * self.zoom + self.pan_x, y * self.zoom + self.pan_y)
    }

    pub fn to_world(&self, sx: f32, sy: f32) -> (f32, f32) {
        ((sx - self.pan_x) / self.zoom, (sy - self.pan_y) / self.zoom)
    }
}

pub fn layout(board: &Board) -> Layout {
    layout_seeded(board, None)
}

pub fn layout_seeded(board: &Board, prev: Option<&Layout>) -> Layout {
    let mut nodes: Vec<&Node> = board.nodes.values().collect();
    nodes.sort_by(|a, b| a.key.cmp(&b.key));
    let n = nodes.len();
    if n == 0 {
        return Layout::default();
    }

    let mut counts = [0u32; 5];
    let mut seen = [0u32; 5];
    for node in &nodes {
        counts[lane_idx(node.lane)] += 1;
    }

    let prev_pos: BTreeMap<NodeKey, (f32, f32)> = prev
        .map(|layout| {
            layout
                .nodes
                .iter()
                .map(|n| (n.key.clone(), (n.x, n.y)))
                .collect()
        })
        .unwrap_or_default();
    let mut pos: Vec<(f32, f32)> = Vec::with_capacity(n);
    for node in &nodes {
        let li = lane_idx(node.lane);
        let i = seen[li];
        seen[li] += 1;
        if node.lane == Lane::Prompt {
            pos.push((0.0, 0.0));
            continue;
        }
        if let Some(&old) = prev_pos.get(&node.key) {
            pos.push(old);
            continue;
        }
        let ring = theme::RING * (li as f32 + 0.15);
        let angle = (i as f32 + 0.37) / counts[li].max(1) as f32 * TAU + li as f32 * 0.31;
        pos.push((angle.cos() * ring, angle.sin() * ring));
    }

    let index: BTreeMap<NodeKey, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, node)| (node.key.clone(), i))
        .collect();
    let mut edges = Vec::new();
    for edge in &board.edges {
        if let (Some(&a), Some(&b)) = (index.get(&edge.from), index.get(&edge.to)) {
            if a != b {
                edges.push((a, b));
            }
        }
    }

    let radii: Vec<f32> = nodes.iter().map(|node| node_radius(node)).collect();
    let k = theme::RING * theme::EDGE_K;
    let k2 = k * k;
    let iters = if prev_pos.is_empty() {
        theme::ITERS
    } else {
        28
    };
    for _ in 0..iters {
        let mut disp = vec![(0.0f32, 0.0f32); n];
        for i in 0..n {
            for j in (i + 1)..n {
                let dx = pos[i].0 - pos[j].0;
                let dy = pos[i].1 - pos[j].1;
                let d2 = dx * dx + dy * dy + 0.08;
                let d = d2.sqrt();
                let mut f = k2 / d;
                let min_d = radii[i] + radii[j] + theme::CLEAR;
                if d < min_d {
                    f += (min_d - d) * 4.2;
                }
                let ux = dx / d * f;
                let uy = dy / d * f;
                disp[i].0 += ux;
                disp[i].1 += uy;
                disp[j].0 -= ux;
                disp[j].1 -= uy;
            }
        }
        for &(a, b) in &edges {
            let dx = pos[a].0 - pos[b].0;
            let dy = pos[a].1 - pos[b].1;
            let d2 = dx * dx + dy * dy + 0.08;
            let d = d2.sqrt();
            let f = d2 / k;
            let ux = dx / d * f;
            let uy = dy / d * f;
            disp[a].0 -= ux;
            disp[a].1 -= uy;
            disp[b].0 += ux;
            disp[b].1 += uy;
        }
        for i in 0..n {
            disp[i].0 -= pos[i].0 * theme::GRAVITY;
            disp[i].1 -= pos[i].1 * theme::GRAVITY;
            let mut dx = disp[i].0 * 0.10;
            let mut dy = disp[i].1 * 0.10;
            let step = (dx * dx + dy * dy).sqrt();
            if step > 5.0 {
                dx *= 5.0 / step;
                dy *= 5.0 / step;
            }
            pos[i].0 += dx * 0.99;
            pos[i].1 += dy * 0.99;
            if nodes[i].lane == Lane::Prompt {
                pos[i] = (0.0, 0.0);
            }
        }
    }

    let placed = nodes
        .iter()
        .enumerate()
        .map(|(i, node)| Placed {
            key: node.key.clone(),
            label: node.label.clone(),
            x: pos[i].0,
            y: pos[i].1,
            r: node_radius(node),
            fill: lane_color(node.lane),
            ring: verdict_ring(node),
        })
        .collect();

    Layout {
        nodes: placed,
        edges,
    }
}

pub fn hit_test(layout: &Layout, world: (f32, f32), pad: f32) -> Option<usize> {
    let (x, y) = world;
    let mut best = None;
    let mut best_d = f32::MAX;
    for (i, n) in layout.nodes.iter().enumerate() {
        let dx = n.x - x;
        let dy = n.y - y;
        let d = (dx * dx + dy * dy).sqrt();
        if d <= n.r + pad && d < best_d {
            best = Some(i);
            best_d = d;
        }
    }
    best
}

pub fn write_png(layout: &Layout, w: u32, h: u32, path: &Path) -> anyhow::Result<()> {
    let cam = Camera::fitted(layout, w as f32, h as f32);
    let mut img = RgbaImage::from_pixel(w, h, rgba(theme::CANVAS));
    for &(a, b) in &layout.edges {
        let (x0, y0) = cam.to_screen(layout.nodes[a].x, layout.nodes[a].y);
        let (x1, y1) = cam.to_screen(layout.nodes[b].x, layout.nodes[b].y);
        draw_line(&mut img, x0, y0, x1, y1, rgba(theme::EDGE));
    }
    for n in &layout.nodes {
        let (sx, sy) = cam.to_screen(n.x, n.y);
        let r = (n.r * cam.zoom).max(2.0);
        fill_circle(&mut img, sx, sy, r, rgba(n.fill));
        draw_ring(&mut img, sx, sy, r + 1.1, rgba(n.ring));
    }
    img.save(path)?;
    Ok(())
}

fn node_radius(node: &Node) -> f32 {
    if node.lane == Lane::Prompt {
        theme::CHAT_R
    } else {
        theme::NODE_MIN + (node.tally.contacts as f32).sqrt() * theme::NODE_GAIN
    }
}

fn lane_idx(lane: Lane) -> usize {
    match lane {
        Lane::Prompt => 0,
        Lane::Playbooks => 1,
        Lane::Skills => 2,
        Lane::Principles => 3,
        Lane::Models => 4,
    }
}

fn lane_color(lane: Lane) -> Color32 {
    match lane {
        Lane::Prompt => theme::PROMPT,
        Lane::Playbooks => theme::PLAYBOOK,
        Lane::Skills => theme::SKILL,
        Lane::Principles => theme::PRINCIPLE,
        Lane::Models => theme::MODEL,
    }
}

fn verdict_ring(node: &Node) -> Color32 {
    if node.tally.hit > node.tally.miss && node.tally.hit > 0 {
        theme::HIT
    } else if node.tally.miss > node.tally.hit {
        theme::MISS
    } else if node.tally.unknown > 0 {
        theme::UNK
    } else {
        theme::MUTED
    }
}

fn rgba(c: Color32) -> Rgba<u8> {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    Rgba([r, g, b, a])
}

fn draw_ring(img: &mut RgbaImage, cx: f32, cy: f32, r: f32, color: Rgba<u8>) {
    let steps = (r * 8.0).max(12.0) as i32;
    for i in 0..steps {
        let a = i as f32 / steps as f32 * TAU;
        let x = cx + a.cos() * r;
        let y = cy + a.sin() * r;
        let px = x.round() as i32;
        let py = y.round() as i32;
        if px >= 0 && py >= 0 && (px as u32) < img.width() && (py as u32) < img.height() {
            img.put_pixel(px as u32, py as u32, color);
        }
    }
}

fn fill_circle(img: &mut RgbaImage, cx: f32, cy: f32, r: f32, color: Rgba<u8>) {
    let w = img.width() as i32;
    let h = img.height() as i32;
    let r2 = r * r;
    let min_x = (cx - r).floor() as i32;
    let max_x = (cx + r).ceil() as i32;
    let min_y = (cy - r).floor() as i32;
    let max_y = (cy + r).ceil() as i32;
    for y in min_y.max(0)..=max_y.min(h - 1) {
        for x in min_x.max(0)..=max_x.min(w - 1) {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            if dx * dx + dy * dy <= r2 {
                img.put_pixel(x as u32, y as u32, color);
            }
        }
    }
}

fn draw_line(img: &mut RgbaImage, x0: f32, y0: f32, x1: f32, y1: f32, color: Rgba<u8>) {
    let mut x = x0;
    let mut y = y0;
    let dx = x1 - x0;
    let dy = y1 - y0;
    let steps = dx.abs().max(dy.abs()).max(1.0) as i32;
    let sx = dx / steps as f32;
    let sy = dy / steps as f32;
    let w = img.width();
    let h = img.height();
    for _ in 0..=steps {
        let px = x.round() as i32;
        let py = y.round() as i32;
        if px >= 0 && py >= 0 && (px as u32) < w && (py as u32) < h {
            img.put_pixel(px as u32, py as u32, color);
        }
        x += sx;
        y += sy;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Board, DocRef, Edge, Node, NodeKey, Tally};

    fn node(label: &str, lane: Lane) -> Node {
        let key = match lane {
            Lane::Models => NodeKey::Model(crate::domain::ModelRef::Requested(label.into())),
            Lane::Playbooks => NodeKey::Doc(DocRef::Playbook(label.into())),
            Lane::Principles => NodeKey::Doc(DocRef::Principle(label.into())),
            Lane::Skills => NodeKey::Doc(DocRef::Skill(label.into())),
            Lane::Prompt => NodeKey::Prompt,
        };
        Node {
            key,
            lane,
            label: label.into(),
            tally: Tally {
                hit: 1,
                miss: 0,
                unknown: 0,
                contacts: 2,
            },
            ambient: 0,
        }
    }

    #[test]
    fn same_board_same_positions() {
        let mut board = Board::default();
        for (label, lane) in [
            ("how", Lane::Skills),
            ("feature", Lane::Playbooks),
            ("architect", Lane::Skills),
        ] {
            let n = node(label, lane);
            board.nodes.insert(n.key.clone(), n);
        }
        board.edges.push(Edge {
            from: NodeKey::Doc(DocRef::Playbook("feature".into())),
            to: NodeKey::Doc(DocRef::Skill("how".into())),
        });
        let a = layout(&board);
        let b = layout(&board);
        assert_eq!(a.nodes.len(), b.nodes.len());
        for (l, r) in a.nodes.iter().zip(b.nodes.iter()) {
            assert!((l.x - r.x).abs() < 1e-4);
            assert!((l.y - r.y).abs() < 1e-4);
        }
    }
}
