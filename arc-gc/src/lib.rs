#![feature(get_mut_unchecked)]
#[macro_use]
extern crate lazy_static;
use std::{
    collections::BTreeMap,
    ops::Deref,
    sync::{Arc, Weak},
};

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}

pub struct ArcGc<X>(Arc<Option<X>>);
impl<X: Send + Sync> ArcGc<X> {
    pub fn new(x: X) -> Self {
        ArcGc(Arc::new(Some(x)))
    }
    pub fn downgrade(&self) -> WeakArcGc<X> {
        WeakArcGc(Arc::downgrade(&self.0))
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
    static ref ALLOW_CYCLES_SET1: BTreeMap<usize, Box<dyn AnyWeakArcGc>> = BTreeMap::new();
    static ref ALLOW_CYCLES_SET2: BTreeMap<usize, Box<dyn AnyWeakArcGc>> = BTreeMap::new();
}
