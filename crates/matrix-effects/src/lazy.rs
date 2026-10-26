use matrix_core::{Effect, Frame, ParameterDescriptor, RenderContext};

/// Function pointer that instantiates a boxed Effect.
pub type EffectFactory = fn() -> Box<dyn Effect>;

/// Lazy wrapper around an Effect factory that defers runtime instance allocation
/// until the first render call, and drops the runtime on `deactivate()`.
/// Metadata and parameter configurations are preserved across activation cycles.
pub struct LazyEffect {
    factory: EffectFactory,
    id: &'static str,
    name: &'static str,
    parameters: Vec<ParameterDescriptor>,
    params: Vec<(&'static str, f32)>,
    inner: Option<Box<dyn Effect>>,
}

impl LazyEffect {
    /// Constructs a `LazyEffect` using the given factory function pointer.
    /// A single temporary instance is instantiated to extract metadata and default values,
    /// and is immediately dropped so no runtime effect remains allocated.
    pub fn new(factory: EffectFactory) -> Self {
        let tmp = factory();
        let id = tmp.id();
        let name = tmp.name();
        let parameters = tmp.parameters().to_vec();
        let mut params = Vec::with_capacity(parameters.len());
        for p in &parameters {
            let val = tmp.parameter(p.id).unwrap_or(p.default);
            params.push((p.id, val));
        }
        drop(tmp);

        Self {
            factory,
            id,
            name,
            parameters,
            params,
            inner: None,
        }
    }

    /// Returns true if the inner effect runtime is currently allocated in RAM.
    #[inline]
    pub fn is_active(&self) -> bool {
        self.inner.is_some()
    }

    fn ensure_inner(&mut self) -> &mut Box<dyn Effect> {
        if self.inner.is_none() {
            let mut inst = (self.factory)();
            for (id, val) in &self.params {
                let _ = inst.set_parameter(id, *val);
            }
            self.inner = Some(inst);
        }
        self.inner.as_mut().unwrap()
    }
}

impl Effect for LazyEffect {
    fn id(&self) -> &'static str {
        self.id
    }

    fn name(&self) -> &'static str {
        self.name
    }

    fn parameters(&self) -> &[ParameterDescriptor] {
        &self.parameters
    }

    fn parameter(&self, id: &str) -> Option<f32> {
        if let Some(inner) = self.inner.as_ref() {
            inner.parameter(id)
        } else {
            self.params.iter().find(|(k, _)| *k == id).map(|(_, v)| *v)
        }
    }

    fn set_parameter(&mut self, id: &str, value: f32) -> Result<(), String> {
        if let Some(inner) = self.inner.as_mut() {
            inner.set_parameter(id, value)?;
        } else {
            let desc = self
                .parameters
                .iter()
                .find(|p| p.id == id)
                .ok_or_else(|| format!("effect '{}' has no parameter '{}'", self.id, id))?;
            desc.validate_value(value)?;
        }

        if let Some(entry) = self.params.iter_mut().find(|(k, _)| *k == id) {
            entry.1 = value;
        }
        Ok(())
    }

    fn reset(&mut self) {
        if let Some(inner) = self.inner.as_mut() {
            inner.reset();
        }
    }

    fn deactivate(&mut self) {
        if let Some(mut inner) = self.inner.take() {
            inner.deactivate();
        }
    }

    fn render(&mut self, frame: &mut Frame, context: &RenderContext<'_>) {
        let inner = self.ensure_inner();
        inner.render(frame, context);
    }
}
