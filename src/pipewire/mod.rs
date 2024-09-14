use json::JsonValue;
use log::{error, warn};
use pw::metadata::Metadata;
use pw::node::Node;
use pw::types::ObjectType;
use spa::param::ParamType;
use spa::pod::deserialize::PodDeserializer;
use spa::pod::{Value, ValueArray};
use spa::sys::{SPA_PROP_channelVolumes, SPA_PROP_mute};
use std::error::Error;
use std::rc::Rc;

pub mod data;
use data::Data;

pub type VolumeInformation = Option<(f32, bool)>;

pub fn listen_for_volume_change(
    volume_listener: impl Fn(VolumeInformation) + 'static,
) -> Result<(), Box<dyn Error>> {
    pw::init();
    let main_loop = pw::main_loop::MainLoop::new(None)?;

    let _sig_int = {
        let main_loop_weak = main_loop.downgrade();
        main_loop
            .loop_()
            .add_signal_local(pw::loop_::Signal::SIGINT, move || {
                if let Some(main_loop) = main_loop_weak.upgrade() {
                    main_loop.quit();
                }
            })
    };

    let _sig_term = {
        let main_loop_weak = main_loop.downgrade();
        main_loop
            .loop_()
            .add_signal_local(pw::loop_::Signal::SIGTERM, move || {
                if let Some(main_loop) = main_loop_weak.upgrade() {
                    main_loop.quit();
                }
            })
    };

    let context = pw::context::Context::new(&main_loop)?;
    let core = context.connect(None)?;
    let registry = Rc::new(core.get_registry()?);

    let _listener_core = {
        let main_loop_weak = main_loop.downgrade();

        core.add_listener_local()
            .done(move |_id, _seq| {})
            .error(move |id, seq, res, message| {
                error!("error id:{} seq:{} res:{}: {}", id, seq, res, message);

                if id == 0 {
                    if let Some(main_loop) = main_loop_weak.upgrade() {
                        main_loop.quit();
                    }
                }
            })
            .register()
    };

    let data = Data::new(volume_listener);

    let _listener = {
        let registry_weak = Rc::downgrade(&registry);
        let data = data.clone();
        registry
            .add_listener_local()
            .global(move |obj| {
                if let Some(registry) = registry_weak.upgrade() {
                    match obj.type_ {
                        ObjectType::Metadata => {
                            let metadata: Metadata = registry.bind(obj).unwrap();
                            let metadata_name = obj.props.map(|props| props.get("metadata.name"));
                            if metadata_name.is_some_and(|name| name.is_some_and(|name| name == "default")) {
                                let listener_metadata = {
                                    let data = data.downgrade();
                                    metadata.add_listener_local()
                                        .property(move |_subject, key, type_, value| {
                                            if key == Some("default.audio.sink") && type_ == Some("Spa:String:JSON") {
                                                if let Some(json_object) = value {
                                                    let value = json::parse(json_object).expect("failed to parse default audio sink json data");
                                                    let name = if let JsonValue::Object(object) = value {
                                                        object.get("name")
                                                            .expect("default audio sink object does not contain name")
                                                            .as_str()
                                                            .expect("default audio sink name is not a string")
                                                            .to_string()
                                                    } else {
                                                        panic!("default audio sink data is not a json object")
                                                    };
                                                    if let Some(data) = data.upgrade() {
                                                        data.set_default_sink(name);
                                                    }
                                                }
                                            }
                                            0
                                        })
                                        .register()
                                };

                                data.track_metadata(metadata, listener_metadata);
                            }
                        }
                        ObjectType::Node => {
                            let node: Node = registry.bind(obj).unwrap();
                            let node_id = obj.id;
                            if let Some(Some(name)) = obj.props.map(|props| if props.get("device.id").is_some() { props.get("node.name") } else { None }) {
                                let node_listener = {
                                    let name = name.to_string();
                                    let data = data.downgrade();
                                    node.add_listener_local()
                                        .param(move |_seq, _id, _index, _next, param| {
                                            if let Some(pod) = param {
                                                let (_rest, value) = PodDeserializer::deserialize_any_from(pod.as_bytes())
                                                    .expect("could not construct deserializer for pod");
                                                match value {
                                                    Value::Object(object) => {
                                                        let mut volume = None;
                                                        let mut muted = None;
                                                        for property in object.properties {
                                                            if property.key == SPA_PROP_channelVolumes {
                                                                match &property.value {
                                                                    Value::ValueArray(ValueArray::Float(array)) => {
                                                                            volume = Some(array[0]);
                                                                    },
                                                                    _ => error!("channel volumes are not a float array"),
                                                                }
                                                            }
                                                            if property.key == SPA_PROP_mute {
                                                                match &property.value {
                                                                    Value::Bool(status) =>  {
                                                                        muted = Some(*status);
                                                                    },
                                                                    _ => error!("channel mute is not a bool"),
                                                                }
                                                            }
                                                        }
                                                        match (data.upgrade(), volume, muted) {
                                                            (Some(data), Some(volume), Some(muted)) => {
                                                                data.set_volume_for_node(name.clone(), volume, muted);
                                                            },
                                                            (_, None, _) => {
                                                                warn!("no channel volumes for node {node_id}");
                                                            },
                                                            (_, _, None) => {
                                                                warn!("no muted status for node {node_id}");
                                                            },
                                                            _ => (),
                                                        }
                                                    }
                                                    _ => error!("node parameter is not an object"),
                                                }
                                            }
                                        })
                                        .register()
                                };
                                node.subscribe_params(&[ParamType::Props]);
                                data.track_node(node, name.to_string(), node_listener);
                            }
                        }
                        _ => (),
                    };
                }
            })
            .register()
    };

    main_loop.run();

    unsafe {
        pw::deinit();
    }

    Ok(())
}
