use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Mutex;

use pipewire as pw;
use pw::spa::utils::dict::DictRef;

pub const METADATA_NAME: &str = "default";
pub const SINK_KEY: &str = "default.audio.sink";

pub struct Handles {
    _metadata: pw::metadata::Metadata,
    _listener: pw::metadata::MetadataListener,
}

#[derive(Default)]
pub struct Watch {
    name: Mutex<Option<String>>,
    handles: RefCell<Option<Handles>>,
}

impl Watch {
    pub fn current(&self) -> Option<String> {
        self.name.lock().ok().and_then(|slot| slot.clone())
    }

    pub fn bind(
        self: &Rc<Self>,
        registry: &pw::registry::RegistryRc,
        object: &pw::registry::GlobalObject<&DictRef>,
        own_nodes: [&str; 2],
    ) {
        let Ok(metadata) = registry.bind::<pw::metadata::Metadata, _>(object) else {
            return;
        };
        let watch = Rc::clone(self);
        let own = own_nodes.map(str::to_owned);
        let listener = metadata
            .add_listener_local()
            .property(move |_subject, key, _type, value| {
                if let (Some(key), Some(value)) = (key, value)
                    && key == SINK_KEY
                    && let Some(name) = parse_sink(value)
                    && !own.iter().any(|node| node == &name)
                    && let Ok(mut slot) = watch.name.lock()
                {
                    *slot = Some(name);
                }
                0
            })
            .register();
        *self.handles.borrow_mut() = Some(Handles {
            _metadata: metadata,
            _listener: listener,
        });
    }
}

pub fn want_metadata(object: &pw::registry::GlobalObject<&DictRef>) -> bool {
    object.type_ == pw::types::ObjectType::Metadata
        && object.props.and_then(|props| props.get("metadata.name")) == Some(METADATA_NAME)
}

pub fn parse_sink(value: &str) -> Option<String> {
    let rest = value.split_once("\"name\"")?.1;
    let rest = rest.split_once('"')?.1;
    let (name, _) = rest.split_once('"')?;
    (!name.is_empty()).then(|| name.to_owned())
}

pub fn watch() -> Rc<Watch> {
    Rc::new(Watch::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sink_name_is_read_from_the_metadata_document() {
        assert_eq!(
            parse_sink("{\"name\":\"bluez_output.aa.1\"}").as_deref(),
            Some("bluez_output.aa.1")
        );
        assert_eq!(
            parse_sink("{ \"name\": \"speakers\" }").as_deref(),
            Some("speakers")
        );
    }

    #[test]
    fn a_document_without_a_name_yields_nothing() {
        assert!(parse_sink("{}").is_none());
        assert!(parse_sink("{\"name\":\"\"}").is_none());
        assert!(parse_sink("not json").is_none());
    }
}
