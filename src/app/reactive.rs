use leptos::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Signal<T>(RwSignal<T>);

impl<T> Signal<T> {
    pub fn new(value: T) -> Self {
        Signal(RwSignal::new(value))
    }

    pub fn get(self) -> T
    where
        T: Copy,
    {
        self.0.get()
    }

    pub fn set(self, new_value: T) {
        self.0.set(new_value)
    }

    pub fn update<F>(self, f: F)
    where
        F: FnOnce(&mut T),
    {
        self.0.update(f)
    }
}

pub fn signal<T>(value: T) -> Signal<T> {
    Signal::new(value)
}
