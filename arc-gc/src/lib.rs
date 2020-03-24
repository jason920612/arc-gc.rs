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

pub struct Gc<X>(Arc<Option<X>>);
impl<X: 'static + Send + Sync + TraceGc> Gc<X> {
    pub fn new(x: X) -> Self {
        Gc(Arc::new(Some(x)))
    }
    pub fn downgrade(&self) -> WeakGc<X> {
        WeakGc(Arc::downgrade(&self.0))
    }
    pub fn mark_allow_cycles(&self) {
        let x = Box::new(self.downgrade());
        ALLOW_CYCLES_MARKER.lock().unwrap().send(x).unwrap();
    }
}
impl<X> Deref for Gc<X> {
    type Target = X;
    fn deref(&self) -> &Self::Target {
        match &*self.0 {
            Some(x) => x,
            None => panic!("destoried ArcGc"),
        }
    }
}
impl<X: TraceGc> TraceGc for Gc<X> {
    fn trace_as_vec(&self) -> Vec<Box<dyn AnyGc>> {
        let this: &X = &*self;
        this.trace_as_vec()
    }
}
pub unsafe trait AnyGc: Send + Sync + TraceGc {
    unsafe fn destory(&self);
    fn address(&self) -> usize;
}
unsafe impl<X: Send + Sync + TraceGc> AnyGc for Gc<X> {
    unsafe fn destory(&self) {
        *Arc::get_mut_unchecked(&mut self.0.clone()) = None;
    }
    fn address(&self) -> usize {
        let pointer: *const Option<X> = &*self.0;
        pointer as usize
    }
}
pub trait TraceGc {
    fn trace_as_vec(&self) -> Vec<Box<dyn AnyGc>>;
}
pub struct WeakGc<X>(Weak<Option<X>>);
impl<X: Send + Sync> WeakGc<X> {
    pub fn new() -> WeakGc<X> {
        WeakGc(Weak::new())
    }
    pub fn upgrade(&self) -> Option<Gc<X>> {
        Some(Gc(self.0.upgrade()?))
    }
}
pub unsafe trait AnyWeakGc: Send + Sync {
    fn any_upgrade(&self) -> Option<Box<dyn AnyGc>>;
}
unsafe impl<X: 'static + Send + Sync + TraceGc> AnyWeakGc for WeakGc<X> {
    fn any_upgrade(&self) -> Option<Box<dyn AnyGc>> {
        Some(Box::new(self.upgrade()?))
    }
}

macro_rules! spawn_auto_sleep_loop_thread {
    ($min:expr,$max:expr,$didworks:ident,$blk:block) => {
        thread::spawn(move || loop {
            let mut sleep_time: u64 = 1;
            loop {
                let mut $didworks = false;
                $blk;
                if $didworks {
                    sleep_time >>= 1;
                } else {
                    sleep_time <<= 1;
                }
                sleep_time = cmp::min(cmp::max(sleep_time, $min), $max);
                thread::sleep(time::Duration::from_millis(sleep_time));
            }
        })
    };
}
lazy_static! {
    static ref ALLOW_CYCLES_SET: Mutex<(
        BTreeMap<usize, Box<dyn AnyWeakGc>>,
        BTreeMap<usize, Box<dyn AnyWeakGc>>
    )> = Mutex::new((BTreeMap::new(), BTreeMap::new()));
    static ref ALLOW_CYCLES_MARKER: Mutex<Sender<Box<dyn AnyWeakGc>>> = {
        spawn_auto_sleep_loop_thread!(1, 64, did_some_works, {
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
        let (sender, receiver) = channel::<Box<dyn AnyWeakGc>>();
        thread::spawn(move || {
            let d = time::Duration::from_millis(100);
            loop {
                if let Ok(x) = receiver.recv_timeout(d) {
                    let x_arc = x.any_upgrade();
                    if let Some(x_arc) = x_arc {
                        ALLOW_CYCLES_SET
                            .lock()
                            .unwrap()
                            .0
                            .insert(x_arc.address(), x);
                    }
                }
            }
        });
        Mutex::new(sender)
    };
}
