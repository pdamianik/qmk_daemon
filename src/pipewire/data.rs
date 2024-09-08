use pw::metadata::{Metadata, MetadataListener};
use pw::node::{Node, NodeListener};
use pw::proxy::{Listener, ProxyT};
use pw::types::ObjectType;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::rc::{Rc, Weak};

struct Inner<L: Fn(Option<f32>) + 'static> {
    default_sink: Option<String>,
    volume: Option<f32>,

    volume_listener: L,

    node_to_volume: HashMap<String, f32>,

    proxies_t: HashMap<u32, Box<dyn ProxyT>>,
    listeners: HashMap<u32, Vec<Box<dyn Listener>>>,
}

impl<Listener: Fn(Option<f32>) + 'static> Debug for Inner<Listener> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Inner")
            .field("default_sink", &self.default_sink)
            .field("volume", &self.volume)
            .field("node_to_volume", &self.node_to_volume)
            .field("proxies_t", &self.proxies_t.iter().map(|(id, proxy_t)| format!("{id}: {:?}", proxy_t.upcast_ref().get_type())).collect::<Vec<_>>())
            .field("listeners", &self.listeners.len())
            .finish()
    }
}

impl<L: Fn(Option<f32>) + 'static> Inner<L> {
    fn add_listener(&mut self, metadata_id: u32, listener: impl Listener + 'static) {
        let v = self.listeners.entry(metadata_id).or_default();
        v.push(Box::new(listener));
    }

    fn remove(&mut self, metadata_id: u32, type_: &ObjectType) {
        match type_ {
            ObjectType::Node => {}
            _ => {
                self.proxies_t.remove(&metadata_id);
            }
        }
        self.listeners.remove(&metadata_id);
    }
}

pub struct Data<Listener: Fn(Option<f32>) + 'static> {
    inner: Rc<RefCell<Inner<Listener>>>,
}

impl<Listener: Fn(Option<f32>) + 'static> Clone for Data<Listener> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<L: Fn(Option<f32>) + 'static> Data<L> {
    pub fn new(volume_listener: L) -> Self {
        Self {
            inner: Rc::new(RefCell::new(Inner {
                default_sink: None,
                volume: None,

                node_to_volume: HashMap::new(),
                volume_listener,

                proxies_t: HashMap::new(),
                listeners: HashMap::new(),
            }))
        }
    }

    pub fn downgrade(&self) -> DataWeak<L> {
        DataWeak {
            inner: Rc::downgrade(&self.inner)
        }
    }

    pub fn track_metadata(&self, metadata: Metadata, listener: MetadataListener) {
        self.track_any(metadata, Some(listener), || ());
    }

    pub fn set_default_sink(&self, default_sink: String) {
        {
            let mut inner = self.inner.borrow_mut();
            if let Some(previous) = &inner.default_sink {
                if *previous == default_sink {
                    return;
                }
            };
            inner.default_sink = Some(default_sink);
        }

        self.check_default_sink();
    }

    pub fn track_node(&self, node: Node, name: String, listener: NodeListener) {
        self.check_default_sink();

        let inner_weak = Rc::downgrade(&self.inner);
        self.track_any(node, Some(listener), move || {
            if let Some(inner) = inner_weak.upgrade() {
                inner.borrow_mut().node_to_volume.remove(&name);
            }
        });
    }

    pub fn set_volume_for_node(&self, node: String, volume: f32) {
        self.inner.borrow_mut().node_to_volume.insert(node.clone(), volume);
        if Some(node) == self.inner.borrow().default_sink {
            self.check_default_sink();
        }
    }

    fn check_default_sink(&self) {
        {
            let mut inner = self.inner.borrow_mut();
            let new_volume = {
                if let Some(sink) = &inner.default_sink {
                    inner.node_to_volume.get(sink).cloned()
                } else {
                    None
                }
            };
            if new_volume != inner.volume {
                // volume changed
                inner.volume = new_volume;
                (inner.volume_listener)(new_volume);
            }
        }
    }

    fn track_any(&self, proxy_t: impl ProxyT + 'static, listener: Option<impl Listener + 'static>, on_remove: impl Fn() + 'static) {
        let proxy_id = self.track_proxy(&proxy_t, on_remove);

        let mut inner = self.inner.borrow_mut();
        inner.proxies_t.insert(proxy_id, Box::new(proxy_t));
        if let Some(listener) = listener {
            inner.add_listener(proxy_id, listener);
        }
    }

    fn track_proxy(&self, proxy_t: &impl ProxyT, on_remove: impl Fn() + 'static) -> u32 {
        let proxy = proxy_t.upcast_ref();
        let proxy_id = proxy.id();
        let proxy_type = proxy.get_type().0;
        let inner_weak = Rc::downgrade(&self.inner);

        let proxy_listener = proxy
            .add_listener_local()
            .removed(move || {
                on_remove();
                if let Some(proxies) = inner_weak.upgrade() {
                    proxies.borrow_mut().remove(proxy_id, &proxy_type);
                }
            })
            .register();

        self.inner.borrow_mut().add_listener(proxy_id, proxy_listener);
        proxy_id
    }
}

pub struct DataWeak<Listener: Fn(Option<f32>) + 'static> {
    inner: Weak<RefCell<Inner<Listener>>>,
}

impl<Listener: Fn(Option<f32>) + 'static> DataWeak<Listener> {
    pub fn upgrade(&self) -> Option<Data<Listener>> {
        if let Some(inner) = self.inner.upgrade() {
            Some(Data {
                inner
            })
        } else {
            None
        }
    }
}
