#[cfg(test)]
use super::*;

#[test]
fn test_simple_graph() {
    let mut graph = FrameGraph::new();

    let swapchain = graph.add_texture(TextureDesc {
        format: vk::Format::B8G8R8A8_SRGB,
        extent: vk::Extent2D { width: 1920, height: 1080 },
        usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
        view: None,
        image: None,
        persistent: true,
    });

    let depth = graph.add_texture(TextureDesc {
        format: vk::Format::D32_SFLOAT,
        extent: vk::Extent2D { width: 1920, height: 1080 },
        usage: vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
        view: None,
        image: None,
        persistent: false,
    });

    // 3D scene pass
    graph.add_pass(PassDesc {
        name: "scene",
        queue: PassQueue::Graphics,
        color_attachments: vec![swapchain],
        depth_attachment: Some(depth),
        sampled_textures: vec![],
        clear_color: true,
        clear_depth: true,
        clear_value: [0.04, 0.04, 0.08, 1.0],
    }, |_ctx| {});

    // UI overlay pass
    graph.add_pass(PassDesc {
        name: "ui",
        queue: PassQueue::Graphics,
        color_attachments: vec![swapchain],
        depth_attachment: None,
        sampled_textures: vec![],
        clear_color: false,
        clear_depth: false,
        clear_value: [0.0; 4],
    }, |_ctx| {});

    graph.compile();

    assert_eq!(graph.pass_count(), 2);

    // Scene pass should begin with UNDEFINED and transition to COLOR_ATTACHMENT_OPTIMAL
    assert!(!graph.pass_barriers_before(0).is_empty());
}

#[test]
fn test_no_barrier_when_layout_unchanged() {
    let mut graph = FrameGraph::new();
    let tex = graph.add_texture(TextureDesc {
        format: vk::Format::B8G8R8A8_SRGB,
        extent: vk::Extent2D { width: 100, height: 100 },
        usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
        view: None,
        image: None,
        persistent: false,
    });

    graph.add_pass(PassDesc {
        name: "p1",
        queue: PassQueue::Graphics,
        color_attachments: vec![tex], depth_attachment: None,
        sampled_textures: vec![], clear_color: true, clear_depth: false,
        clear_value: [0.0; 4],
    }, |_ctx| {});
    graph.add_pass(PassDesc {
        name: "p2",
        queue: PassQueue::Graphics,
        color_attachments: vec![tex], depth_attachment: None,
        sampled_textures: vec![], clear_color: false, clear_depth: false,
        clear_value: [0.0; 4],
    }, |_ctx| {});
    graph.compile();

    // Second pass should have NO barriers since layout is already COLOR_ATTACHMENT_OPTIMAL
    assert!(graph.pass_barriers_before(1).is_empty());
}

#[test]
fn test_merge_compatible_passes() {
    let mut graph = FrameGraph::new();
    let tex = graph.add_texture(TextureDesc {
        format: vk::Format::B8G8R8A8_SRGB,
        extent: vk::Extent2D { width: 100, height: 100 },
        usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
        view: None,
        image: None,
        persistent: false,
    });

    graph.add_pass(PassDesc {
        name: "p1",
        queue: PassQueue::Graphics,
        color_attachments: vec![tex], depth_attachment: None,
        sampled_textures: vec![], clear_color: true, clear_depth: false,
        clear_value: [0.0; 4],
    }, |_ctx| {});
    graph.add_pass(PassDesc {
        name: "p2",
        queue: PassQueue::Graphics,
        color_attachments: vec![tex], depth_attachment: None,
        sampled_textures: vec![], clear_color: false, clear_depth: false,
        clear_value: [0.0; 4],
    }, |_ctx| {});
    graph.compile();

    // Both passes use the same attachment with no barriers between them -> merged
    assert_eq!(graph.merged_groups().len(), 1);
    assert_eq!(graph.merged_groups()[0], (0, 1));
}

#[test]
fn test_no_merge_when_barriers_present() {
    let mut graph = FrameGraph::new();
    let tex = graph.add_texture(TextureDesc {
        format: vk::Format::B8G8R8A8_SRGB,
        extent: vk::Extent2D { width: 100, height: 100 },
        usage: vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
        view: None,
        image: None,
        persistent: false,
    });

    // Pass 1 writes to tex
    graph.add_pass(PassDesc {
        name: "write",
        queue: PassQueue::Graphics,
        color_attachments: vec![tex], depth_attachment: None,
        sampled_textures: vec![], clear_color: true, clear_depth: false,
        clear_value: [0.0; 4],
    }, |_ctx| {});
    // Pass 2 reads from tex as sampled texture -> barrier required
    graph.add_pass(PassDesc {
        name: "read",
        queue: PassQueue::Graphics,
        color_attachments: vec![], depth_attachment: None,
        sampled_textures: vec![tex], clear_color: false, clear_depth: false,
        clear_value: [0.0; 4],
    }, |_ctx| {});
    graph.compile();

    // Barrier between passes prevents merging
    assert_eq!(graph.merged_groups().len(), 2);
}

#[test]
fn test_no_merge_when_attachments_differ() {
    let mut graph = FrameGraph::new();
    let tex_a = graph.add_texture(TextureDesc {
        format: vk::Format::B8G8R8A8_SRGB,
        extent: vk::Extent2D { width: 100, height: 100 },
        usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
        view: None,
        image: None,
        persistent: false,
    });
    let tex_b = graph.add_texture(TextureDesc {
        format: vk::Format::B8G8R8A8_SRGB,
        extent: vk::Extent2D { width: 100, height: 100 },
        usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
        view: None,
        image: None,
        persistent: false,
    });

    graph.add_pass(PassDesc {
        name: "p1",
        queue: PassQueue::Graphics,
        color_attachments: vec![tex_a], depth_attachment: None,
        sampled_textures: vec![], clear_color: true, clear_depth: false,
        clear_value: [0.0; 4],
    }, |_ctx| {});
    graph.add_pass(PassDesc {
        name: "p2",
        queue: PassQueue::Graphics,
        color_attachments: vec![tex_b], depth_attachment: None,
        sampled_textures: vec![], clear_color: false, clear_depth: false,
        clear_value: [0.0; 4],
    }, |_ctx| {});
    graph.compile();

    // Different color attachments prevent merging
    assert_eq!(graph.merged_groups().len(), 2);
}

#[test]
fn test_compute_pass_not_merged_with_graphics() {
    let mut graph = FrameGraph::new();
    let tex = graph.add_texture(TextureDesc {
        format: vk::Format::B8G8R8A8_SRGB,
        extent: vk::Extent2D { width: 100, height: 100 },
        usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
        view: None,
        image: None,
        persistent: false,
    });

    // Graphics pass
    graph.add_pass(PassDesc {
        name: "gfx",
        queue: PassQueue::Graphics,
        color_attachments: vec![tex], depth_attachment: None,
        sampled_textures: vec![], clear_color: true, clear_depth: false,
        clear_value: [0.0; 4],
    }, |_ctx| {});
    // Compute pass with same attachment
    graph.add_pass(PassDesc {
        name: "compute",
        queue: PassQueue::Compute,
        color_attachments: vec![tex], depth_attachment: None,
        sampled_textures: vec![], clear_color: false, clear_depth: false,
        clear_value: [0.0; 4],
    }, |_ctx| {});
    graph.compile();

    // Different queues prevent merging
    assert_eq!(graph.merged_groups().len(), 2);
    assert_eq!(graph.merged_groups()[0], (0, 0));
    assert_eq!(graph.merged_groups()[1], (1, 1));
}

#[test]
fn test_compute_passes_merge_with_each_other() {
    let mut graph = FrameGraph::new();
    let tex = graph.add_texture(TextureDesc {
        format: vk::Format::B8G8R8A8_SRGB,
        extent: vk::Extent2D { width: 100, height: 100 },
        usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
        view: None,
        image: None,
        persistent: false,
    });

    graph.add_pass(PassDesc {
        name: "c1",
        queue: PassQueue::Compute,
        color_attachments: vec![tex], depth_attachment: None,
        sampled_textures: vec![], clear_color: true, clear_depth: false,
        clear_value: [0.0; 4],
    }, |_ctx| {});
    graph.add_pass(PassDesc {
        name: "c2",
        queue: PassQueue::Compute,
        color_attachments: vec![tex], depth_attachment: None,
        sampled_textures: vec![], clear_color: false, clear_depth: false,
        clear_value: [0.0; 4],
    }, |_ctx| {});
    graph.compile();

    // Same queue + same attachments + no barriers = merged
    assert_eq!(graph.merged_groups().len(), 1);
    assert_eq!(graph.merged_groups()[0], (0, 1));
}

#[test]
fn test_snapshot_contains_passes_and_barriers() {
    let mut graph = FrameGraph::new();
    let tex = graph.add_texture(TextureDesc {
        format: vk::Format::R8G8B8A8_UNORM,
        extent: vk::Extent2D { width: 64, height: 64 },
        usage: vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
        view: None,
        image: None,
        persistent: false,
    });
    graph.add_pass(PassDesc {
        name: "write",
        queue: PassQueue::Graphics,
        color_attachments: vec![tex], depth_attachment: None,
        sampled_textures: vec![], clear_color: true, clear_depth: false,
        clear_value: [0.0; 4],
    }, |_ctx| {});
    graph.add_pass(PassDesc {
        name: "read",
        queue: PassQueue::Graphics,
        color_attachments: vec![], depth_attachment: None,
        sampled_textures: vec![tex], clear_color: false, clear_depth: false,
        clear_value: [0.0; 4],
    }, |_ctx| {});
    graph.compile();

    let snap = graph.snapshot();
    assert_eq!(snap.passes.len(), 2);
    assert_eq!(snap.passes[0].name, "write");
    assert_eq!(snap.passes[1].name, "read");
    assert_eq!(snap.textures.len(), 1);
    assert_eq!(snap.barriers.len(), 2);
    // Barrier from write -> read should exist on the read pass
    assert!(!snap.barriers[1].before.is_empty());
    assert_eq!(snap.merged_groups.len(), 2);
}
