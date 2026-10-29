use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Mutex;

use pipewire as pw;
use pw::spa::utils::dict::DictRef;
use pwe_core::PipelineKind;

pub const METADATA_NAME: &str = "default";

pub struct Handles {
    _metadata: pw::metadata::Metadata,
    _listener: pw::metadata::MetadataListener,
}

#[derive(Default)]
pub struct Watch {
    name: Mutex<Option<String>>,
    handles: RefCell<Option<Handles>>,
    own: Vec<String>,
}

impl Watch {
    pub fn current(&self) -> Option<String> {
        self.name.lock().ok().and_then(|slot| slot.clone())
    }

    pub fn bind(
        self: &Rc<Self>,
        registry: &pw::registry::RegistryRc,
        object: &pw::registry::GlobalObject<&DictRef>,
        kind: PipelineKind,
    ) {
        let Ok(metadata) = registry.bind::<pw::metadata::Metadata, _>(object) else {
            return;
        };
        let watch = Rc::clone(self);
        let wanted = crate::config::metadata_key(kind);
        let listener = metadata
            .add_listener_local()
            .property(move |_subject, key, _type, value| {
                if let (Some(key), Some(value)) = (key, value)
                    && key == wanted
                    && let Some(name) = parse_sink(value)
                    && !watch.own.contains(&name)
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

pub fn watch(own_nodes: Vec<String>) -> Rc<Watch> {
    Rc::new(Watch {
        own: own_nodes,
        ..Watch::default()
    })
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

    #[test]
    fn each_pipeline_watches_its_own_metadata_key() {
        assert_eq!(
            crate::config::metadata_key(PipelineKind::Output),
            "default.audio.sink"
        );
        assert_eq!(
            crate::config::metadata_key(PipelineKind::Input),
            "default.audio.source"
        );
    }
}
