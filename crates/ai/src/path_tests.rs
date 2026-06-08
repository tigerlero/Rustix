//! Tests for A* pathfinding.

use crate::path::{PathNode, a_star_grid};

#[test]
fn test_grid_path_simple() {
    let w = 3u32; let h = 3u32;
    let blocked = vec![false; (w * h) as usize];
    let pf = a_star_grid(w, h, &blocked);
    let path = pf.find_path(0, 8).unwrap();
    assert!(path.len() >= 3);
    assert_eq!(path[0], 0);
    assert_eq!(path[path.len() - 1], 8);
}

#[test]
fn test_grid_path_blocked() {
    let w = 3u32; let h = 3u32;
    let mut blocked = vec![false; (w * h) as usize];
    blocked[4] = true; // center blocked
    let pf = a_star_grid(w, h, &blocked);
    let path = pf.find_path(0, 8).unwrap();
    assert!(!path.contains(&4));
}

#[test]
fn test_no_path() {
    let w = 2u32; let h = 2u32;
    let mut blocked = vec![false; (w * h) as usize];
    blocked[1] = true; blocked[2] = true;
    let pf = a_star_grid(w, h, &blocked);
    assert!(pf.find_path(0, 3).is_none());
}
