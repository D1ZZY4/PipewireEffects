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
    pub fn label(self) -> &'static str {
        pwe_core::i18n::t(match self {
            Self::Dynamics => "Dynamics",
            Self::Equalisation => "Equalisation",
            Self::Tone => "Tone and colour",
            Self::Time => "Time based",
            Self::Analysis => "Analysis",
        })
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
        id: "midside_equalizer",
        label: "Mid/side equalizer",
        summary: "Ten band equaliser applied to the middle and the side separately",
        category: EffectCategory::Equalisation,
        build: |params| Box::new(effects::MidSideEqualizer::new(params)),
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
        id: "expander",
        label: "Expander",
        summary: "Downward expander that widens the dynamic range below a threshold",
        category: EffectCategory::Dynamics,
        build: |params| Box::new(effects::Expander::new(params)),
    },
    EffectDescriptor {
        id: "maximizer",
        label: "Maximizer",
        summary: "Automatic peak maximiser with a ceiling and a release stage",
        category: EffectCategory::Dynamics,
        build: |params| Box::new(effects::Maximizer::new(params)),
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
        id: "crusher",
        label: "Crusher",
        summary: "Bit depth and sample rate reduction with a modulated hold",
        category: EffectCategory::Tone,
        build: |params| Box::new(effects::Crusher::new(params)),
    },
    EffectDescriptor {
        id: "crossfeed",
        label: "Crossfeed",
        summary: "Mixes the channels so headphones sound like speakers",
        category: EffectCategory::Tone,
        build: |params| Box::new(effects::Crossfeed::new(params)),
    },
    EffectDescriptor {
        id: "crosstalk",
        label: "Crosstalk canceller",
        summary: "Delayed reciprocal mix that puts a stereo image back in front of you",
        category: EffectCategory::Tone,
        build: |params| Box::new(effects::Crosstalk::new(params)),
    },
    EffectDescriptor {
        id: "exciter",
        label: "Exciter",
        summary: "Harmonic generation in a band above the scope frequency",
        category: EffectCategory::Tone,
        build: |params| Box::new(effects::Exciter::new(params)),
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
        "midside_equalizer" => effects::midside_equalizer_specs(),
        "compressor" => effects::compressor_specs(),
        "limiter" => effects::limiter_specs(),
        "gate" => effects::gate_specs(),
        "expander" => effects::expander_specs(),
        "maximizer" => effects::maximizer_specs(),
        "deesser" => effects::deesser_specs(),
        "autogain" => effects::autogain_specs(),
        "bass_enhancer" => effects::bass_enhancer_specs(),
        "crystalizer" => effects::crystalizer_specs(),
        "crusher" => effects::crusher_specs(),
        "stereo_tools" => effects::stereo_tools_specs(),
        "crossfeed" => effects::crossfeed_specs(),
        "crosstalk" => effects::crosstalk_specs(),
        "exciter" => effects::exciter_specs(),
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

pub fn rebuild(id: &str, params: ParamSet) -> Option<Box<dyn Effect>> {
    let descriptor = find(id)?;
    Some((descriptor.build)(params))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_descriptor_has_unique_identity_and_parameters() {
        let mut ids: Vec<&str> = Vec::new();
        let mut labels: Vec<&str> = Vec::new();
        for descriptor in EFFECTS {
            assert!(
                !ids.contains(&descriptor.id),
                "{} is listed twice",
                descriptor.id
            );
            assert!(
                !labels.contains(&descriptor.label),
                "{} shares its label",
                descriptor.id
            );
            assert!(
                !descriptor.summary.is_empty(),
                "{} has no summary",
                descriptor.id
            );
            ids.push(descriptor.id);
            labels.push(descriptor.label);
        }
    }

    #[test]
    fn every_descriptor_publishes_parameters_and_builds() {
        for descriptor in EFFECTS {
            let specs = descriptor.specs();
            assert!(
                !specs.is_empty(),
                "{} publishes no parameters",
                descriptor.id
            );
            let params = ParamSet::new(specs);
            let mut effect = (descriptor.build)(params.clone());
            assert_eq!(effect.id(), descriptor.id);
            effect.prepare(pwe_core::AudioFormat::new(48_000, 2));
            effect.reset();
        }
    }

    #[test]
    fn every_parameter_identifier_is_unique_within_an_effect() {
        for descriptor in EFFECTS {
            let mut seen: Vec<&str> = Vec::new();
            for spec in descriptor.specs() {
                assert!(
                    !seen.contains(&spec.id),
                    "{} lists {} twice",
                    descriptor.id,
                    spec.id
                );
                seen.push(spec.id);
            }
        }
    }

    #[test]
    fn an_unknown_identifier_is_not_instantiated() {
        assert!(find("nonesuch").is_none());
        assert!(specs_for("nonesuch").is_none());
        assert!(instantiate("nonesuch").is_none());
    }
}
