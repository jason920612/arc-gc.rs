#![feature(get_mut_unchecked)]
#[macro_use]
extern crate lazy_static;
use std::{
    collections::BTreeMap,
    ops::Deref,
    sync::{
        mpsc::{channel, Sender},
        Arc, Mutex, Weak,
    },
    thread,
};

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}

pub struct ArcGc<X>(Arc<Option<X>>);
impl<X: 'static + Send + Sync> ArcGc<X> {
    pub fn new(x: X) -> Self {
        ArcGc(Arc::new(Some(x)))
    }
    pub fn downgrade(&self) -> WeakArcGc<X> {
        WeakArcGc(Arc::downgrade(&self.0))
    }
    pub fn mark_allow_cycles(&self) {
        ALLOW_CYCLES_MARKER
            .lock()
            .unwrap()
            .send(Box::new(self.downgrade()))
            .unwrap();
    }
}
impl<X> Deref for ArcGc<X> {
    type Target = X;
    fn deref(&self) -> &Self::Target {
        match &*self.0 {
            Some(x) => x,
            None => panic!("destoried ArcGc"),
        }
    }
}
pub unsafe trait AnyArcGc: Send + Sync {
    unsafe fn destory(&self);
    fn address(&self) -> usize;
}
unsafe impl<X: Send + Sync> AnyArcGc for ArcGc<X> {
    unsafe fn destory(&self) {
        *Arc::get_mut_unchecked(&mut self.0.clone()) = None;
    }
    fn address(&self) -> usize {
        let pointer: *const Option<X> = &*self.0;
        pointer as usize
    }
}
pub struct WeakArcGc<X>(Weak<Option<X>>);
impl<X: Send + Sync> WeakArcGc<X> {
    pub fn new() -> WeakArcGc<X> {
        WeakArcGc(Weak::new())
    }
    pub fn upgrade(&self) -> Option<ArcGc<X>> {
        Some(ArcGc(self.0.upgrade()?))
    }
}
pub unsafe trait AnyWeakArcGc: Send + Sync {
    fn any_upgrade(&self) -> Option<Box<dyn AnyArcGc>>;
}
unsafe impl<X: 'static + Send + Sync> AnyWeakArcGc for WeakArcGc<X> {
    fn any_upgrade(&self) -> Option<Box<dyn AnyArcGc>> {
        Some(Box::new(self.upgrade()?))
    }
}

lazy_static! {
    static ref ALLOW_CYCLES_SET: Mutex<(
        BTreeMap<usize, Box<dyn AnyWeakArcGc>>,
        BTreeMap<usize, Box<dyn AnyWeakArcGc>>
    )> = Mutex::new((BTreeMap::new(), BTreeMap::new()));
    static ref ALLOW_CYCLES_MARKER: Mutex<Sender<Box<dyn AnyWeakArcGc>>> = {
        let (sender, receiver) = channel::<Box<dyn AnyWeakArcGc>>();
        thread::spawn(move || loop {
            let x = receiver.recv().unwrap();
            let x_arc = x.any_upgrade();
            if let Some(x_arc) = x_arc {
                ALLOW_CYCLES_SET
                    .lock()
                    .unwrap()
                    .0
                    .insert(x_arc.address(), x);
            }
        });
        Mutex::new(sender)
    };
}
