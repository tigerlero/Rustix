import re

with open("apps/runtime/src/ui_renderer.rs", "r") as f:
    content = f.read()

content = content.replace('''                if !self.descriptor_pool.is_null() {
                    dev.destroy_descriptor_pool(self.descriptor_pool, None);
                }''', '''                for pool in &self.descriptor_pools {
                    let p = *pool.lock().unwrap();
                    if !p.is_null() {
                        dev.destroy_descriptor_pool(p, None);
                    }
                }''')

with open("apps/runtime/src/ui_renderer.rs", "w") as f:
    f.write(content)
