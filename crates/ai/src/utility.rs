//! Utility AI: scoring actions based on response curves.
//!
//! Each action has a utility score computed by evaluating one or more
//! response curves against normalized input values (0..1). The highest
//! scoring action is selected.

/// A response curve that maps a normalized input [0, 1] to a utility
/// score [0, 1].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Curve {
    /// Linear: y = x
    Linear,
    /// Exponential: y = x^exp
    Exponential { exp: f32 },
    /// Logistic sigmoid: y = 1 / (1 + e^(-steepness * (x - offset)))
    Sigmoid { steepness: f32, offset: f32 },
    /// Step: y = 1 if x >= threshold else 0
    Step { threshold: f32 },
    /// Inverse linear: y = 1 - x
    Inverse,
}

impl Curve {
    /// Evaluate the curve at `x` (clamped to [0, 1]).
    pub fn evaluate(&self, x: f32) -> f32 {
        let x = x.clamp(0.0, 1.0);
        match *self {
            Curve::Linear => x,
            Curve::Exponential { exp } => x.powf(exp),
            Curve::Sigmoid { steepness, offset } => {
                1.0 / (1.0 + (-steepness * (x - offset)).exp())
            }
            Curve::Step { threshold } => if x >= threshold { 1.0 } else { 0.0 },
            Curve::Inverse => 1.0 - x,
        }
    }
}

/// A single consideration that scores an input value through a curve.
#[derive(Debug, Clone)]
pub struct Consideration {
    pub name: String,
    pub curve: Curve,
    pub weight: f32,
}

impl Consideration {
    pub fn new(name: impl Into<String>, curve: Curve, weight: f32) -> Self {
        Self {
            name: name.into(),
            curve,
            weight,
        }
    }

    pub fn score(&self, input: f32) -> f32 {
        self.curve.evaluate(input) * self.weight
    }
}

/// An action in the utility system composed of multiple considerations.
#[derive(Debug, Clone)]
pub struct UtilityAction {
    pub name: String,
    pub considerations: Vec<Consideration>,
}

impl UtilityAction {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            considerations: Vec::new(),
        }
    }

    pub fn with(mut self, consideration: Consideration) -> Self {
        self.considerations.push(consideration);
        self
    }

    /// Compute the total utility score for this action.
    ///
    /// `inputs` is a map from consideration name to normalized input value.
    /// Missing inputs are treated as 0.0.
    pub fn score(&self, inputs: &std::collections::HashMap<String, f32>) -> f32 {
        let mut total = 0.0f32;
        let mut weight_sum = 0.0f32;
        for c in &self.considerations {
            let input = inputs.get(&c.name).copied().unwrap_or(0.0);
            total += c.score(input);
            weight_sum += c.weight;
        }
        if weight_sum > 0.0 {
            total / weight_sum
        } else {
            0.0
        }
    }
}

/// Utility reasoner that scores actions and picks the best one.
#[derive(Debug, Clone)]
pub struct UtilityReasoner {
    pub actions: Vec<UtilityAction>,
}

impl UtilityReasoner {
    pub fn new(actions: Vec<UtilityAction>) -> Self {
        Self { actions }
    }

    /// Evaluate all actions with the given inputs and return the highest
    /// scoring action name along with its score.
    pub fn select(&self, inputs: &std::collections::HashMap<String, f32>) -> Option<(&str, f32)> {
        self.actions
            .iter()
            .map(|a| (a.name.as_str(), a.score(inputs)))
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    }

    /// Return all actions sorted by score (highest first).
    pub fn ranked(&self, inputs: &std::collections::HashMap<String, f32>) -> Vec<(&str, f32)> {
        let mut ranked: Vec<_> = self.actions.iter().map(|a| (a.name.as_str(), a.score(inputs))).collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        ranked
    }
}
