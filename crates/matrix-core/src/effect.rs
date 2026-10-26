use crate::audio::AudioSnapshot;
use crate::pixel::{Frame, Rgb};
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterKind {
    Number,
    Boolean,
    Select,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct ParameterDescriptor {
    pub id: &'static str,
    pub label: &'static str,
    pub kind: ParameterKind,
    pub min: f32,
    pub max: f32,
    pub step: f32,
    pub default: f32,
    pub options: &'static [&'static str],
    pub unit: &'static str,
}

impl ParameterDescriptor {
    pub fn validate_value(&self, value: f32) -> Result<(), String> {
        if !value.is_finite() {
            return Err(format!("parameter '{}' value must be finite", self.id));
        }
        if value < self.min || value > self.max {
            return Err(format!(
                "parameter '{}' value {} out of range [{}, {}]",
                self.id, value, self.min, self.max
            ));
        }
        match self.kind {
            ParameterKind::Boolean => {
                if value == 0.0 || value == 1.0 {
                    Ok(())
                } else {
                    Err(format!(
                        "parameter '{}' boolean value must be 0 or 1, got {}",
                        self.id, value
                    ))
                }
            }
            ParameterKind::Select => {
                let idx = value as usize;
                if value.fract() != 0.0 || value < 0.0 || idx >= self.options.len() {
                    Err(format!(
                        "parameter '{}' select index {} out of options bounds (len {})",
                        self.id,
                        value,
                        self.options.len()
                    ))
                } else {
                    Ok(())
                }
            }
            ParameterKind::Number => {
                if self.step > 0.0 {
                    let steps = (value - self.min) / self.step;
                    if (steps - steps.round()).abs() > 0.001 {
                        return Err(format!(
                            "parameter '{}' value {} does not align with step {}",
                            self.id, value, self.step
                        ));
                    }
                }
                Ok(())
            }
        }
    }

    /// Parses an incoming text payload (from MQTT or API) into a validated float parameter value.
    pub fn parse_value(&self, payload: &str) -> Result<f32, String> {
        let trimmed = payload.trim();
        match self.kind {
            ParameterKind::Boolean => {
                if trimmed.eq_ignore_ascii_case("on")
                    || trimmed.eq_ignore_ascii_case("true")
                    || trimmed == "1"
                {
                    Ok(1.0)
                } else if trimmed.eq_ignore_ascii_case("off")
                    || trimmed.eq_ignore_ascii_case("false")
                    || trimmed == "0"
                {
                    Ok(0.0)
                } else {
                    Err(format!("invalid boolean payload '{}'", payload))
                }
            }
            ParameterKind::Select => {
                // Check if string matches any option
                if let Some(pos) = self
                    .options
                    .iter()
                    .position(|&opt| opt.eq_ignore_ascii_case(trimmed))
                {
                    return Ok(pos as f32);
                }
                // Check if it's an integer index
                if let Ok(idx) = trimmed.parse::<usize>() {
                    if idx < self.options.len() {
                        return Ok(idx as f32);
                    }
                }
                Err(format!(
                    "invalid select payload '{}' for parameter '{}', expected one of {:?}",
                    payload, self.id, self.options
                ))
            }
            ParameterKind::Number => {
                let val = trimmed
                    .parse::<f32>()
                    .map_err(|e| format!("invalid number payload '{}': {}", payload, e))?;
                self.validate_value(val)?;
                Ok(val)
            }
        }
    }
}

pub struct RenderContext<'a> {
    pub now_ms: u64,
    pub delta_seconds: f32,
    pub audio: &'a AudioSnapshot,
    pub color: Rgb,
    pub text: &'a str,
    pub scroll_speed: f32,
}

pub trait Effect: Send {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn parameters(&self) -> &[ParameterDescriptor] {
        &[]
    }
    fn parameter(&self, _id: &str) -> Option<f32> {
        None
    }
    fn set_parameter(&mut self, id: &str, _value: f32) -> Result<(), String> {
        Err(format!("effect '{}' has no parameter '{}'", self.id(), id))
    }
    fn reset(&mut self) {}
    /// Release transient rendering resources while retaining effect settings.
    fn deactivate(&mut self) {}
    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>);
}
