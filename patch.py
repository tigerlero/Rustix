import re

with open("apps/runtime/src/ui_renderer.rs", "r") as f:
    content = f.read()

content = content.replace("descriptor_set: vk::DescriptorSet,", "")
content = content.replace("descriptor_pool: vk::DescriptorPool,", "descriptor_pools: [std::sync::Mutex<vk::DescriptorPool>; 3],\n    last_frame_index: Cell<usize>,\n    bound_descriptor_set: Cell<vk::DescriptorSet>,")

content = content.replace("descriptor_set:desc_set,descriptor_pool:desc_pool", "descriptor_pools:desc_pools,last_frame_index:Cell::new(usize::MAX),bound_descriptor_set:Cell::new(vk::DescriptorSet::null())")

with open("apps/runtime/src/ui_renderer.rs", "w") as f:
    f.write(content)
