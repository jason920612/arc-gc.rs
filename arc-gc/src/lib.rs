#![feature(get_mut_unchecked)]

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}

use std::{ops::Deref, sync::Arc};

pub struct ArcGc<X>(Arc<Option<X>>);
impl<X> ArcGc<X> {
    pub fn new(x: X) -> Self {
        ArcGc(Arc::new(Some(x)))
    }
    unsafe fn destory(&self) {
        let mut this = self.0.clone();
        *Arc::get_mut_unchecked(&mut this) = None;
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
