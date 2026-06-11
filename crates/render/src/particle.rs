use rustix_core::math::{Vec3, Vec4};

/// Per-particle data stored in a GPU instance buffer.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct ParticleInstance {
    pub position: Vec3,
    pub size: f32,
    pub color: Vec4,
}

unsafe impl bytemuck::Pod for ParticleInstance {}
unsafe impl bytemuck::Zeroable for ParticleInstance {}

/// GPU particle state used by compute simulation and billboard rendering.
/// Layout: pos_size (16) + color (16) + velocity (16) + params (16) = 64 bytes.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct GpuParticle {
    /// xyz = world position, w = current size.
    pub pos_size: Vec4,
    /// rgba color.
    pub color: Vec4,
    /// xyz = velocity, w = remaining lifetime.
    pub velocity: Vec4,
    /// x = max_lifetime, y = start_size, z = end_size, w = alive (1.0 or 0.0).
    pub params: Vec4,
}

unsafe impl bytemuck::Pod for GpuParticle {}
unsafe impl bytemuck::Zeroable for GpuParticle {}

impl GpuParticle {
    pub fn new(position: Vec3, size: f32, color: Vec4, velocity: Vec3, lifetime: f32, max_lifetime: f32, start_size: f32, end_size: f32) -> Self {
        Self {
            pos_size: Vec4::new(position.x, position.y, position.z, size),
            color,
            velocity: Vec4::new(velocity.x, velocity.y, velocity.z, lifetime),
            params: Vec4::new(max_lifetime, start_size, end_size, 1.0),
        }
    }

    pub fn is_alive(&self) -> bool {
        self.params.w > 0.5
    }

    pub fn set_alive(&mut self, alive: bool) {
        self.params.w = if alive { 1.0 } else { 0.0 };
    }
}

/// CPU-side particle state before upload to GPU.
#[derive(Debug, Clone)]
pub struct Particle {
    pub position: Vec3,
    pub velocity: Vec3,
    pub lifetime: f32,
    pub max_lifetime: f32,
    pub size: f32,
    pub color: Vec4,
    pub active: bool,
}

impl Default for Particle {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            lifetime: 0.0,
            max_lifetime: 1.0,
            size: 0.1,
            color: Vec4::ONE,
            active: false,
        }
    }
}

/// CPU particle simulation for a single emitter.
pub struct ParticleSimulation {
    pub particles: Vec<Particle>,
    pub spawn_accumulator: f32,
}

impl ParticleSimulation {
    pub fn new(max_particles: usize) -> Self {
        Self {
            particles: vec![Particle::default(); max_particles],
            spawn_accumulator: 0.0,
        }
    }

    pub fn update(
        &mut self,
        dt: f32,
        emitter_pos: Vec3,
        spawn_rate: f32,
        velocity: Vec3,
        velocity_spread: f32,
        lifetime: f32,
        lifetime_spread: f32,
        start_size: f32,
        end_size: f32,
        start_color: [f32; 4],
        end_color: [f32; 4],
        gravity: Vec3,
    ) -> Vec<ParticleInstance> {
        // Update existing particles
        for p in self.particles.iter_mut() {
            if p.active {
                p.velocity += gravity * dt;
                p.position += p.velocity * dt;
                p.lifetime -= dt;
                let t = 1.0 - (p.lifetime / p.max_lifetime).clamp(0.0, 1.0);
                p.size = start_size + (end_size - start_size) * t;
                p.color = Vec4::new(
                    start_color[0] + (end_color[0] - start_color[0]) * t,
                    start_color[1] + (end_color[1] - start_color[1]) * t,
                    start_color[2] + (end_color[2] - start_color[2]) * t,
                    start_color[3] + (end_color[3] - start_color[3]) * t,
                );
                if p.lifetime <= 0.0 {
                    p.active = false;
                }
            }
        }

        // Spawn new particles
        self.spawn_accumulator += spawn_rate * dt;
        let spawn_count = self.spawn_accumulator.floor() as u32;
        self.spawn_accumulator -= spawn_count as f32;

        let particle_count = self.particles.len() as u32;
        let mut spawned = 0u32;
        for p in self.particles.iter_mut() {
            if spawned >= spawn_count {
                break;
            }
            if !p.active {
                p.active = true;
                p.position = emitter_pos;
                let mut rng_seed = self.spawn_accumulator.to_bits() ^ particle_count;
                let mut next_rand = || {
                    rng_seed = rng_seed.wrapping_mul(1103515245).wrapping_add(12345);
                    (rng_seed as f32 / u32::MAX as f32)
                };
                let sx = velocity.x + (next_rand() - 0.5) * velocity_spread;
                let sy = velocity.y + (next_rand() - 0.5) * velocity_spread;
                let sz = velocity.z + (next_rand() - 0.5) * velocity_spread;
                p.velocity = Vec3::new(sx, sy, sz);
                p.max_lifetime = lifetime + (next_rand() - 0.5) * lifetime_spread;
                p.lifetime = p.max_lifetime;
                p.size = start_size;
                p.color = Vec4::new(start_color[0], start_color[1], start_color[2], start_color[3]);
                spawned += 1;
            }
        }

        // Build GPU instance data from active particles
        self.particles
            .iter()
            .filter(|p| p.active)
            .map(|p| ParticleInstance {
                position: p.position,
                size: p.size,
                color: p.color,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn particle_sim_spawns_particles() {
        let mut sim = ParticleSimulation::new(10);
        let instances = sim.update(
            1.0, Vec3::ZERO, 5.0, Vec3::Y, 0.0, 1.0, 0.0,
            0.1, 0.01, [1.0; 4], [1.0, 1.0, 1.0, 0.0], Vec3::ZERO,
        );
        assert_eq!(instances.len(), 5, "should spawn 5 particles in one second at 5/sec");
    }

    #[test]
    fn particle_sim_kills_after_lifetime() {
        let mut sim = ParticleSimulation::new(10);
        sim.update(
            1.0, Vec3::ZERO, 10.0, Vec3::ZERO, 0.0, 0.5, 0.0,
            0.1, 0.01, [1.0; 4], [1.0, 1.0, 1.0, 0.0], Vec3::ZERO,
        );
        let instances = sim.update(
            1.0, Vec3::ZERO, 0.0, Vec3::ZERO, 0.0, 0.5, 0.0,
            0.1, 0.01, [1.0; 4], [1.0, 1.0, 1.0, 0.0], Vec3::ZERO,
        );
        assert_eq!(instances.len(), 0, "all particles should die after lifetime");
    }

    #[test]
    fn particle_sim_gravity_affects_position() {
        let mut sim = ParticleSimulation::new(1);
        sim.update(
            1.0, Vec3::ZERO, 1.0, Vec3::ZERO, 0.0, 10.0, 0.0,
            0.1, 0.01, [1.0; 4], [1.0; 4], Vec3::new(0.0, -10.0, 0.0),
        );
        let pos_before = sim.particles[0].position;
        let instances = sim.update(
            1.0, Vec3::ZERO, 0.0, Vec3::ZERO, 0.0, 10.0, 0.0,
            0.1, 0.01, [1.0; 4], [1.0; 4], Vec3::new(0.0, -10.0, 0.0),
        );
        assert!(!instances.is_empty());
        assert!(sim.particles[0].position.y < pos_before.y, "gravity should pull particle down");
    }

    #[test]
    fn particle_sim_size_interpolates() {
        let mut sim = ParticleSimulation::new(1);
        sim.update(
            1.0, Vec3::ZERO, 1.0, Vec3::ZERO, 0.0, 2.0, 0.0,
            1.0, 0.1, [1.0; 4], [1.0; 4], Vec3::ZERO,
        );
        let start_size = sim.particles[0].size;
        sim.update(
            1.0, Vec3::ZERO, 0.0, Vec3::ZERO, 0.0, 2.0, 0.0,
            1.0, 0.1, [1.0; 4], [1.0; 4], Vec3::ZERO,
        );
        let mid_size = sim.particles[0].size;
        assert!(mid_size < start_size, "size should shrink over lifetime");
        assert!(mid_size > 0.1, "size should not yet reach end size");
    }
}
