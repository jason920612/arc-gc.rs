#![feature(get_mut_unchecked)]
#![feature(map_first_last)]
#[macro_use]
extern crate lazy_static;
use std::{
    cmp,
    collections::BTreeMap,
    ops::Deref,
    sync::{
        mpsc::{channel, Sender},
        Arc, Mutex, Weak,
    },
    thread, time,
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
        let x = Box::new(self.downgrade());
        ALLOW_CYCLES_MARKER.lock().unwrap().send(x).unwrap();
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

macro_rules! auto_sleep_loop {
    ($didworks:ident,$blk:block) => {
        let mut sleep_time: u64 = 1;
        loop {
            let mut $didworks = false;
            $blk;
            if $didworks {
                sleep_time >>= 1;
            } else {
                sleep_time <<= 1;
            }
            sleep_time = cmp::min(cmp::max(sleep_time, 1), 256);
            thread::sleep(time::Duration::from_millis(sleep_time));
        }
    };
}
lazy_static! {
    static ref ALLOW_CYCLES_SET: Mutex<(
        BTreeMap<usize, Box<dyn AnyWeakArcGc>>,
        BTreeMap<usize, Box<dyn AnyWeakArcGc>>
    )> = Mutex::new((BTreeMap::new(), BTreeMap::new()));
    static ref ALLOW_CYCLES_MARKER: Mutex<Sender<Box<dyn AnyWeakArcGc>>> = {
        thread::spawn(move || {
            auto_sleep_loop!(did_some_works, {
                let mut locked = ALLOW_CYCLES_SET.lock().unwrap();
                if let Some(entry) = locked.0.first_entry() {
                    let (key, val) = entry.remove_entry();
                    if let Some(val_arc) = val.any_upgrade() {
                        let addr = val_arc.address();
                        assert_eq!(addr, key);
                        locked.1.insert(val_arc.address(), val);
                    }
                    did_some_works = true;
                } else {
                    if !locked.1.is_empty() {
                        let mut new_locked0 = BTreeMap::new();
                        new_locked0.append(&mut locked.1);
                        assert!(locked.1.is_empty());
                        locked.0 = new_locked0;
                        assert!(!locked.0.is_empty());
                        did_some_works = true;
                    }
                }
            });
        });
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
