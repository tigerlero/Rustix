//! Rustix AI system: behavior trees, pathfinding, navigation.

pub mod btree;
pub mod path;
pub mod nav;
pub mod steering;
pub mod fsm;
pub mod sensor;
pub mod goap;
pub mod utility;
pub mod influence;
pub mod debug_draw;

#[cfg(test)]
pub mod steering_tests;
#[cfg(test)]
pub mod fsm_tests;
#[cfg(test)]
pub mod sensor_tests;
#[cfg(test)]
pub mod goap_tests;
#[cfg(test)]
pub mod influence_tests;
#[cfg(test)]
pub mod utility_tests;
#[cfg(test)]
pub mod btree_tests;
#[cfg(test)]
pub mod debug_draw_tests;
#[cfg(test)]
pub mod path_tests;
#[cfg(test)]
pub mod nav_tests;

pub use btree::{BehaviorTree, Blackboard, Status, BehaviorNode, Action, Condition, Sequence, Selector, Parallel, Invert, Repeat};
pub use path::{PathFinder, PathNode, a_star_grid, a_star_graph};
pub use nav::{NavMesh, NavTriangle, NavMeshSource, NavMeshGenerator};
pub use steering::{Agent, seek, flee, arrive, wander, avoid_obstacles, separation, alignment, cohesion, combine, integrate};
pub use fsm::{Fsm, State, Transition, StateId};
pub use sensor::{VisionCone, HearingRadius, AgentSensor};
pub use goap::{GoapPlanner, GoapAction, WorldState, Fact};
pub use utility::{Curve, Consideration, UtilityAction, UtilityReasoner};
pub use influence::InfluenceMap;
pub use debug_draw::{AiDebugDraw, DebugLine, DebugPoint, DebugLabel};
