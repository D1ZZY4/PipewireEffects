use pwe_core::i18n::mark;
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
            Self::Dynamics => mark("Dynamics"),
            Self::Equalisation => mark("Equalisation"),
            Self::Tone => mark("Tone and colour"),
            Self::Time => mark("Time based"),
            Self::Analysis => mark("Analysis"),
        })
    }
}

pub struct EffectDescriptor {
    pub id: &'static str,
    pub label: &'static str,
    pub summary: &'static str,
    pub category: EffectCategory,
    pub specs: fn() -> Vec<ParamSpec>,
    pub build: fn(ParamSet) -> Box<dyn Effect>,
}

impl EffectDescriptor {
    pub fn specs(&self) -> Vec<ParamSpec> {
        (self.specs)()
    }
}

pub const EFFECTS: &[EffectDescriptor] = &[
    EffectDescriptor {
        id: "equalizer",
        label: mark("Equalizer"),
        summary: mark("Ten band graphic equaliser with an analytic response curve"),
        category: EffectCategory::Equalisation,
        specs: effects::equalizer_specs,
        build: |params| Box::new(effects::Equalizer::new(params)),
    },
    EffectDescriptor {
        id: "filter",
        label: mark("Filter"),
        summary: mark("Low pass, high pass, band pass, notch, shelf, peak and all pass"),
        category: EffectCategory::Equalisation,
        specs: effects::filter_specs,
        build: |params| Box::new(effects::Filter::new(params)),
    },
    EffectDescriptor {
        id: "midside_equalizer",
        label: mark("Mid/side equalizer"),
        summary: mark("Ten band equaliser applied to the middle and the side separately"),
        category: EffectCategory::Equalisation,
        specs: effects::midside_equalizer_specs,
        build: |params| Box::new(effects::MidSideEqualizer::new(params)),
    },
    EffectDescriptor {
        id: "compressor",
        label: mark("Compressor"),
        summary: mark(
            "Feed forward compressor with peak or RMS detection, a soft knee and a filtered sidechain",
        ),
        category: EffectCategory::Dynamics,
        specs: effects::compressor_specs,
        build: |params| Box::new(effects::Compressor::new(params)),
    },
    EffectDescriptor {
        id: "limiter",
        label: mark("Limiter"),
        summary: mark("Lookahead peak limiter with a hard ceiling, a release and a stereo link"),
        category: EffectCategory::Dynamics,
        specs: effects::limiter_specs,
        build: |params| Box::new(effects::Limiter::new(params)),
    },
    EffectDescriptor {
        id: "gate",
        label: mark("Gate"),
        summary: mark("Noise gate with a curve, hysteresis, hold, attack and release"),
        category: EffectCategory::Dynamics,
        specs: effects::gate_specs,
        build: |params| Box::new(effects::Gate::new(params)),
    },
    EffectDescriptor {
        id: "multiband_compressor",
        label: mark("Multiband compressor"),
        summary: mark("Split the spectrum and compress each band separately"),
        category: EffectCategory::Dynamics,
        specs: effects::multiband_compressor_specs,
        build: |params| Box::new(effects::MultibandCompressor::new(params)),
    },
    EffectDescriptor {
        id: "multiband_gate",
        label: mark("Multiband gate"),
        summary: mark("Split the spectrum and gate each band separately"),
        category: EffectCategory::Dynamics,
        specs: effects::multiband_gate_specs,
        build: |params| Box::new(effects::MultibandGate::new(params)),
    },
    EffectDescriptor {
        id: "expander",
        label: mark("Expander"),
        summary: mark(
            "Downward expander that widens the dynamic range below a threshold, with a filtered sidechain",
        ),
        category: EffectCategory::Dynamics,
        specs: effects::expander_specs,
        build: |params| Box::new(effects::Expander::new(params)),
    },
    EffectDescriptor {
        id: "maximizer",
        label: mark("Maximizer"),
        summary: mark("Automatic peak maximiser with a ceiling and a release stage"),
        category: EffectCategory::Dynamics,
        specs: effects::maximizer_specs,
        build: |params| Box::new(effects::Maximizer::new(params)),
    },
    EffectDescriptor {
        id: "deesser",
        label: mark("De-esser"),
        summary: mark("Split or wideband sibilance reduction"),
        category: EffectCategory::Dynamics,
        specs: effects::deesser_specs,
        build: |params| Box::new(effects::DeEsser::new(params)),
    },
    EffectDescriptor {
        id: "autogain",
        label: mark("Auto gain"),
        summary: mark("EBU R128 loudness normalisation towards a target level"),
        category: EffectCategory::Dynamics,
        specs: effects::autogain_specs,
        build: |params| Box::new(effects::AutoGain::new(params)),
    },
    EffectDescriptor {
        id: "bass_enhancer",
        label: mark("Bass enhancer"),
        summary: mark("Adds harmonics to the low end so small speakers reproduce it"),
        category: EffectCategory::Tone,
        specs: effects::bass_enhancer_specs,
        build: |params| Box::new(effects::BassEnhancer::new(params)),
    },
    EffectDescriptor {
        id: "crystalizer",
        label: mark("Crystalizer"),
        summary: mark("Four band transient emphasis"),
        category: EffectCategory::Tone,
        specs: effects::crystalizer_specs,
        build: |params| Box::new(effects::Crystalizer::new(params)),
    },
    EffectDescriptor {
        id: "crusher",
        label: mark("Crusher"),
        summary: mark("Bit depth and sample rate reduction with a modulated hold"),
        category: EffectCategory::Tone,
        specs: effects::crusher_specs,
        build: |params| Box::new(effects::Crusher::new(params)),
    },
    EffectDescriptor {
        id: "crossfeed",
        label: mark("Crossfeed"),
        summary: mark("Mixes the channels so headphones sound like speakers"),
        category: EffectCategory::Tone,
        specs: effects::crossfeed_specs,
        build: |params| Box::new(effects::Crossfeed::new(params)),
    },
    EffectDescriptor {
        id: "crosstalk",
        label: mark("Crosstalk canceller"),
        summary: mark("Delayed reciprocal mix that puts a stereo image back in front of you"),
        category: EffectCategory::Tone,
        specs: effects::crosstalk_specs,
        build: |params| Box::new(effects::Crosstalk::new(params)),
    },
    EffectDescriptor {
        id: "exciter",
        label: mark("Exciter"),
        summary: mark("Harmonic generation in a band above the scope frequency"),
        category: EffectCategory::Tone,
        specs: effects::exciter_specs,
        build: |params| Box::new(effects::Exciter::new(params)),
    },
    EffectDescriptor {
        id: "stereo_tools",
        label: mark("Stereo tools"),
        summary: mark("Mid/side matrix, seven routing modes, levels, panoramas and a Haas delay"),
        category: EffectCategory::Tone,
        specs: effects::stereo_tools_specs,
        build: |params| Box::new(effects::StereoTools::new(params)),
    },
    EffectDescriptor {
        id: "gain",
        label: mark("Gain"),
        summary: mark("Input gain and mute"),
        category: EffectCategory::Tone,
        specs: effects::gain_specs,
        build: |params| Box::new(effects::Gain::new(params)),
    },
    EffectDescriptor {
        id: "voice_suppressor",
        label: mark("Voice suppressor"),
        summary: mark("Attenuates centred voice by comparing the two channels"),
        category: EffectCategory::Tone,
        specs: effects::voice_suppressor_specs,
        build: |params| Box::new(effects::VoiceSuppressor::new(params)),
    },
    EffectDescriptor {
        id: "noise_reduction",
        label: mark("Noise reduction"),
        summary: mark("Removes steady background noise with a recurrent network at 48 kHz"),
        category: EffectCategory::Analysis,
        specs: effects::noise_reduction_specs,
        build: |params| Box::new(effects::NoiseReduction::new(params)),
    },
    EffectDescriptor {
        id: "delay",
        label: mark("Delay"),
        summary: mark("Independent left and right delay with feedback and ping-pong"),
        category: EffectCategory::Time,
        specs: effects::delay_specs,
        build: |params| Box::new(effects::Delay::new(params)),
    },
    EffectDescriptor {
        id: "reverb",
        label: mark("Reverberation"),
        summary: mark("Freeverb style room with pre-delay, diffusion, damping and a tone stage"),
        category: EffectCategory::Time,
        specs: effects::reverb_specs,
        build: |params| Box::new(effects::Reverb::new(params)),
    },
];

pub fn find(id: &str) -> Option<&'static EffectDescriptor> {
    EFFECTS.iter().find(|descriptor| descriptor.id == id)
}

pub fn instantiate(id: &str) -> Option<(ParamSet, Box<dyn Effect>)> {
    let descriptor = find(id)?;
    let params = ParamSet::new(descriptor.specs());
    let effect = (descriptor.build)(params.clone());
    Some((params, effect))
}

pub fn rebuild(id: &str, params: ParamSet) -> Option<Box<dyn Effect>> {
    let descriptor = find(id)?;
    Some((descriptor.build)(params))
}

#[cfg(test)]
mod tests;
