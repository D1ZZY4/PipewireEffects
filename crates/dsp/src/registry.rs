use pwe_core::{Effect, ParamSet, ParamSpec};

use crate::effects;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectCategory {
    Dynamics,
    Equalisation,
    Tone,
    Time,
    Analysis,
}

impl EffectCategory {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Dynamics => "Dynamics",
            Self::Equalisation => "Equalisation",
            Self::Tone => "Tone and colour",
            Self::Time => "Time based",
            Self::Analysis => "Analysis",
        }
    }
}

pub struct EffectDescriptor {
    pub id: &'static str,
    pub label: &'static str,
    pub summary: &'static str,
    pub category: EffectCategory,
    pub build: fn(ParamSet) -> Box<dyn Effect>,
}

impl EffectDescriptor {
    pub fn specs(&self) -> Vec<ParamSpec> {
        specs_for(self.id).unwrap_or_default()
    }
}

pub const EFFECTS: &[EffectDescriptor] = &[
    EffectDescriptor {
        id: "equalizer",
        label: "Equalizer",
        summary: "Ten band graphic equaliser with an analytic response curve",
        category: EffectCategory::Equalisation,
        build: |params| Box::new(effects::Equalizer::new(params)),
    },
    EffectDescriptor {
        id: "filter",
        label: "Filter",
        summary: "Low pass, high pass, band pass, notch, shelf, peak and all pass",
        category: EffectCategory::Equalisation,
        build: |params| Box::new(effects::Filter::new(params)),
    },
    EffectDescriptor {
        id: "compressor",
        label: "Compressor",
        summary: "Feed forward compressor with peak or RMS detection and a soft knee",
        category: EffectCategory::Dynamics,
        build: |params| Box::new(effects::Compressor::new(params)),
    },
    EffectDescriptor {
        id: "limiter",
        label: "Limiter",
        summary: "Instant attack limiter with a hard ceiling and a release stage",
        category: EffectCategory::Dynamics,
        build: |params| Box::new(effects::Limiter::new(params)),
    },
    EffectDescriptor {
        id: "gate",
        label: "Gate",
        summary: "Noise gate with hold, attack and release",
        category: EffectCategory::Dynamics,
        build: |params| Box::new(effects::Gate::new(params)),
    },
    EffectDescriptor {
        id: "deesser",
        label: "De-esser",
        summary: "Split or wideband sibilance reduction",
        category: EffectCategory::Dynamics,
        build: |params| Box::new(effects::DeEsser::new(params)),
    },
    EffectDescriptor {
        id: "autogain",
        label: "Auto gain",
        summary: "Slow loudness normalisation towards a target level",
        category: EffectCategory::Dynamics,
        build: |params| Box::new(effects::AutoGain::new(params)),
    },
    EffectDescriptor {
        id: "bass_enhancer",
        label: "Bass enhancer",
        summary: "Adds harmonics to the low end so small speakers reproduce it",
        category: EffectCategory::Tone,
        build: |params| Box::new(effects::BassEnhancer::new(params)),
    },
    EffectDescriptor {
        id: "crystalizer",
        label: "Crystalizer",
        summary: "Four band transient emphasis",
        category: EffectCategory::Tone,
        build: |params| Box::new(effects::Crystalizer::new(params)),
    },
    EffectDescriptor {
        id: "stereo_tools",
        label: "Stereo tools",
        summary: "Width, balance and a Haas stereo base",
        category: EffectCategory::Tone,
        build: |params| Box::new(effects::StereoTools::new(params)),
    },
    EffectDescriptor {
        id: "gain",
        label: "Gain",
        summary: "Input gain and mute",
        category: EffectCategory::Tone,
        build: |params| Box::new(effects::Gain::new(params)),
    },
    EffectDescriptor {
        id: "delay",
        label: "Delay",
        summary: "Independent left and right delay with feedback and ping-pong",
        category: EffectCategory::Time,
        build: |params| Box::new(effects::Delay::new(params)),
    },
    EffectDescriptor {
        id: "reverb",
        label: "Reverberation",
        summary: "Freeverb style room with pre-delay, damping and width",
        category: EffectCategory::Time,
        build: |params| Box::new(effects::Reverb::new(params)),
    },
];

pub fn find(id: &str) -> Option<&'static EffectDescriptor> {
    EFFECTS.iter().find(|descriptor| descriptor.id == id)
}

pub fn specs_for(id: &str) -> Option<Vec<ParamSpec>> {
    Some(match id {
        "equalizer" => effects::equalizer_specs(),
        "filter" => effects::filter_specs(),
        "compressor" => effects::compressor_specs(),
        "limiter" => effects::limiter_specs(),
        "gate" => effects::gate_specs(),
        "deesser" => effects::deesser_specs(),
        "autogain" => effects::autogain_specs(),
        "bass_enhancer" => effects::bass_enhancer_specs(),
        "crystalizer" => effects::crystalizer_specs(),
        "stereo_tools" => effects::stereo_tools_specs(),
        "gain" => effects::gain_specs(),
        "delay" => effects::delay_specs(),
        "reverb" => effects::reverb_specs(),
        _ => return None,
    })
}

pub fn instantiate(id: &str) -> Option<(ParamSet, Box<dyn Effect>)> {
    let descriptor = find(id)?;
    let specs = specs_for(id)?;
    let params = ParamSet::new(specs);
    let effect = (descriptor.build)(params.clone());
    Some((params, effect))
}
