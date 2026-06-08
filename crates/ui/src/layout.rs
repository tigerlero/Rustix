//! Flexbox and grid layout engine for the Rustix UI framework.
//!
//! Layout is computed each frame in immediate-mode fashion.
//! Children declare their desired size; the layout engine resolves final positions.

use glam::Vec2;

// ── Layout Configuration ──

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlexDirection {
    Row,
    Column,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Justify {
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Start,
    Center,
    End,
    Stretch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlexWrap {
    NoWrap,
    Wrap,
}

// ── Layout Item ──

/// A single child in a flex or grid container.
#[derive(Debug, Clone, Copy)]
pub struct LayoutItem {
    pub pos: Vec2,
    pub size: Vec2,
    pub grow: f32,
    pub shrink: f32,
    pub basis: Option<f32>,
}

impl LayoutItem {
    pub fn new(size: Vec2) -> Self {
        Self { pos: Vec2::ZERO, size, grow: 0.0, shrink: 1.0, basis: None }
    }

    pub fn with_grow(mut self, grow: f32) -> Self {
        self.grow = grow;
        self
    }

    pub fn with_shrink(mut self, shrink: f32) -> Self {
        self.shrink = shrink;
        self
    }

    pub fn with_basis(mut self, basis: f32) -> Self {
        self.basis = Some(basis);
        self
    }
}

// ── Flex Layout ──

#[derive(Debug, Clone, Copy)]
pub struct FlexLayout {
    pub direction: FlexDirection,
    pub justify: Justify,
    pub align: Align,
    pub wrap: FlexWrap,
    pub gap: f32,
    pub padding: [f32; 4], // top, right, bottom, left
}

impl Default for FlexLayout {
    fn default() -> Self {
        Self {
            direction: FlexDirection::Row,
            justify: Justify::Start,
            align: Align::Start,
            wrap: FlexWrap::NoWrap,
            gap: 0.0,
            padding: [0.0; 4],
        }
    }
}

impl FlexLayout {
    pub fn row() -> Self {
        Self { direction: FlexDirection::Row, ..Default::default() }
    }
    pub fn column() -> Self {
        Self { direction: FlexDirection::Column, ..Default::default() }
    }
    pub fn justify(mut self, j: Justify) -> Self {
        self.justify = j;
        self
    }
    pub fn align(mut self, a: Align) -> Self {
        self.align = a;
        self
    }
    pub fn wrap(mut self) -> Self {
        self.wrap = FlexWrap::Wrap;
        self
    }
    pub fn gap(mut self, g: f32) -> Self {
        self.gap = g;
        self
    }
    pub fn padding(mut self, top: f32, right: f32, bottom: f32, left: f32) -> Self {
        self.padding = [top, right, bottom, left];
        self
    }
}

/// Compute final positions for flex children inside a container.
/// Returns the bounding size of the laid-out content.
pub fn flex_layout(container: Vec2, items: &mut [LayoutItem], config: &FlexLayout) -> Vec2 {
    if items.is_empty() { return Vec2::ZERO; }

    let is_row = config.direction == FlexDirection::Row;
    let main_axis = |v: Vec2| if is_row { v.x } else { v.y };
    let cross_axis = |v: Vec2| if is_row { v.y } else { v.x };

    let available_main = main_axis(container)
        - config.padding[3] - config.padding[1]  // left + right for row, top + bottom for column
        - (items.len().saturating_sub(1) as f32) * config.gap;

    // Resolve basis sizes
    let mut total_basis: f32 = 0.0;
    for item in items.iter_mut() {
        let basis = item.basis.unwrap_or_else(|| main_axis(item.size));
        if is_row {
            item.size.x = basis;
        } else {
            item.size.y = basis;
        }
        total_basis += basis;
    }

    // Distribute remaining space via grow / shrink
    let remaining = available_main - total_basis;
    if remaining > 0.0 {
        let total_grow: f32 = items.iter().map(|i| i.grow).sum();
        if total_grow > 0.0 {
            for item in items.iter_mut() {
                let extra = remaining * (item.grow / total_grow);
                if is_row {
                    item.size.x += extra;
                } else {
                    item.size.y += extra;
                }
            }
        }
    } else if remaining < 0.0 {
        let total_shrink: f32 = items.iter().map(|i| i.shrink).sum();
        if total_shrink > 0.0 {
            for item in items.iter_mut() {
                let reduction = remaining.abs() * (item.shrink / total_shrink);
                if is_row {
                    item.size.x = (item.size.x - reduction).max(0.0);
                } else {
                    item.size.y = (item.size.y - reduction).max(0.0);
                }
            }
        }
    }

    // Compute cross-axis sizes for stretch alignment
    let max_cross = items.iter().map(|i| cross_axis(i.size)).fold(0.0, f32::max);
    if config.align == Align::Stretch {
        for item in items.iter_mut() {
            if is_row {
                item.size.y = max_cross.max(container.y - config.padding[0] - config.padding[2]);
            } else {
                item.size.x = max_cross.max(container.x - config.padding[3] - config.padding[1]);
            }
        }
    }

    let used_main: f32 = items.iter().map(|i| main_axis(i.size)).sum::<f32>()
        + (items.len().saturating_sub(1) as f32) * config.gap;

    // Main-axis positioning (justify)
    let _cursor_main = match config.justify {
        Justify::Start | Justify::SpaceBetween | Justify::SpaceAround | Justify::SpaceEvenly => {
            config.padding[3] // left (row) or top (column via padding[0], but we map left->top for column)
        }
        Justify::Center => {
            (main_axis(container) - used_main) * 0.5
        }
        Justify::End => {
            main_axis(container) - used_main - config.padding[1]
        }
    };

    // For column, top padding is padding[0]
    let cursor_main = if is_row {
        config.padding[3]
    } else {
        config.padding[0]
    };

    let mut cursor_main = match config.justify {
        Justify::Start | Justify::SpaceBetween | Justify::SpaceAround | Justify::SpaceEvenly => cursor_main,
        Justify::Center => (main_axis(container) - used_main) * 0.5,
        Justify::End => main_axis(container) - used_main - if is_row { config.padding[1] } else { config.padding[2] },
    };

    let gap_extra = match config.justify {
        Justify::SpaceBetween if items.len() > 1 => (main_axis(container) - used_main) / (items.len() - 1) as f32,
        Justify::SpaceAround => (main_axis(container) - used_main) / items.len() as f32,
        Justify::SpaceEvenly => (main_axis(container) - used_main) / (items.len() + 1) as f32,
        _ => config.gap,
    };

    for (_i, item) in items.iter_mut().enumerate() {
        if config.justify == Justify::SpaceAround {
            cursor_main += gap_extra * 0.5;
        } else if config.justify == Justify::SpaceEvenly {
            cursor_main += gap_extra;
        }

        // Cross-axis positioning (align)
        let container_cross = cross_axis(container)
            - if is_row { config.padding[0] + config.padding[2] } else { config.padding[3] + config.padding[1] };
        let item_cross = cross_axis(item.size);
        let cursor_cross = match config.align {
            Align::Start => if is_row { config.padding[0] } else { config.padding[3] },
            Align::Center => (container_cross - item_cross) * 0.5 + if is_row { config.padding[0] } else { config.padding[3] },
            Align::End => container_cross - item_cross + if is_row { config.padding[0] } else { config.padding[3] },
            Align::Stretch => if is_row { config.padding[0] } else { config.padding[3] },
        };

        if is_row {
            item.pos = Vec2::new(cursor_main, cursor_cross);
        } else {
            item.pos = Vec2::new(cursor_cross, cursor_main);
        }

        cursor_main += main_axis(item.size) + gap_extra + if matches!(config.justify, Justify::SpaceAround) { 0.0 } else { config.gap };
        // SpaceBetween/Start/End/Center already handled by gap_extra logic above
        if matches!(config.justify, Justify::SpaceAround) {
            cursor_main += gap_extra * 0.5;
        }
    }

    // Return bounding box
    let max_x = items.iter().map(|i| i.pos.x + i.size.x).fold(0.0, f32::max);
    let max_y = items.iter().map(|i| i.pos.y + i.size.y).fold(0.0, f32::max);
    Vec2::new(max_x, max_y)
}

// ── Grid Layout ──

#[derive(Debug, Clone, Copy)]
pub struct GridLayout {
    pub columns: usize,
    pub rows: usize,
    pub col_gap: f32,
    pub row_gap: f32,
    pub padding: [f32; 4], // top, right, bottom, left
    pub justify: Justify,
    pub align: Align,
}

impl Default for GridLayout {
    fn default() -> Self {
        Self {
            columns: 2,
            rows: 2,
            col_gap: 0.0,
            row_gap: 0.0,
            padding: [0.0; 4],
            justify: Justify::Start,
            align: Align::Start,
        }
    }
}

impl GridLayout {
    pub fn new(columns: usize, rows: usize) -> Self {
        Self { columns, rows, ..Default::default() }
    }
    pub fn gap(mut self, col: f32, row: f32) -> Self {
        self.col_gap = col;
        self.row_gap = row;
        self
    }
    pub fn padding(mut self, top: f32, right: f32, bottom: f32, left: f32) -> Self {
        self.padding = [top, right, bottom, left];
        self
    }
    pub fn justify(mut self, j: Justify) -> Self {
        self.justify = j;
        self
    }
    pub fn align(mut self, a: Align) -> Self {
        self.align = a;
        self
    }
}

/// Compute final positions for grid children inside a container.
/// Items are placed left-to-right, top-to-bottom. Returns bounding size.
pub fn grid_layout(container: Vec2, items: &mut [LayoutItem], config: &GridLayout) -> Vec2 {
    if items.is_empty() { return Vec2::ZERO; }

    let cols = config.columns.max(1);
    let content_width = container.x - config.padding[3] - config.padding[1];
    let content_height = container.y - config.padding[0] - config.padding[2];
    let col_w = (content_width - (cols.saturating_sub(1) as f32) * config.col_gap) / cols as f32;

    let rows_needed = (items.len() + cols - 1) / cols;
    let row_h = (content_height - (rows_needed.saturating_sub(1) as f32) * config.row_gap)
        / rows_needed.max(1) as f32;

    for (idx, item) in items.iter_mut().enumerate() {
        let col = idx % cols;
        let row = idx / cols;

        let cell_x = config.padding[3] + col as f32 * (col_w + config.col_gap);
        let cell_y = config.padding[0] + row as f32 * (row_h + config.row_gap);

        // Justify within cell
        let x = match config.justify {
            Justify::Start => cell_x,
            Justify::Center => cell_x + (col_w - item.size.x) * 0.5,
            Justify::End => cell_x + col_w - item.size.x,
            _ => cell_x,
        };

        // Align within cell
        let y = match config.align {
            Align::Start => cell_y,
            Align::Center => cell_y + (row_h - item.size.y) * 0.5,
            Align::End => cell_y + row_h - item.size.y,
            Align::Stretch => {
                item.size.x = col_w;
                item.size.y = row_h;
                cell_y
            }
        };

        item.pos = Vec2::new(x, y);
    }

    let max_x = items.iter().map(|i| i.pos.x + i.size.x).fold(0.0, f32::max);
    let max_y = items.iter().map(|i| i.pos.y + i.size.y).fold(0.0, f32::max);
    Vec2::new(max_x, max_y)
}

// ── Convenience helpers on UIContext ──

use crate::UIContext;

impl UIContext {
    /// Layout children as a flex row. `children` receives each item's computed position.
    pub fn flex_row(&mut self, container_pos: Vec2, container_size: Vec2, config: &FlexLayout,
        mut items: Vec<LayoutItem>, mut children: impl FnMut(&mut UIContext, usize, Vec2, Vec2)) -> Vec2
    {
        let size = flex_layout(container_size, &mut items, config);
        for (i, item) in items.iter().enumerate() {
            children(self, i, container_pos + item.pos, item.size);
        }
        size
    }

    /// Layout children as a flex column.
    pub fn flex_column(&mut self, container_pos: Vec2, container_size: Vec2, config: &FlexLayout,
        mut items: Vec<LayoutItem>, mut children: impl FnMut(&mut UIContext, usize, Vec2, Vec2)) -> Vec2
    {
        let mut cfg = *config;
        cfg.direction = FlexDirection::Column;
        let size = flex_layout(container_size, &mut items, &cfg);
        for (i, item) in items.iter().enumerate() {
            children(self, i, container_pos + item.pos, item.size);
        }
        size
    }

    /// Layout children in a grid.
    pub fn grid(&mut self, container_pos: Vec2, container_size: Vec2, config: &GridLayout,
        mut items: Vec<LayoutItem>, mut children: impl FnMut(&mut UIContext, usize, Vec2, Vec2)) -> Vec2
    {
        let size = grid_layout(container_size, &mut items, config);
        for (i, item) in items.iter().enumerate() {
            children(self, i, container_pos + item.pos, item.size);
        }
        size
    }
}
