use nih_plug::prelude::*;
use std::sync::Arc;

mod dsp;
mod editor;
mod params;

use dsp::WaveformBuffer;
use params::GainParams;

struct Gain {
    params: Arc<GainParams>,
    waveform_buffer: Arc<WaveformBuffer>,
}

impl Default for Gain {
    fn default() -> Self {
        Self {
            params: Arc::new(GainParams::default()),
            waveform_buffer: WaveformBuffer::new(),
        }
    }
}

impl Plugin for Gain {
    const NAME: &'static str = "Quaker Waker";
    const VENDOR: &'static str = "Walkerware";
    const URL: &'static str = "";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            aux_input_ports: &[],
            aux_output_ports: &[],
            names: PortNames::const_default(),
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(1),
            ..AudioIOLayout::const_default()
        },
    ];

    const MIDI_INPUT: MidiConfig = MidiConfig::None;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        editor::create(self.params.clone(), self.waveform_buffer.clone())
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        _buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        true
    }

    fn reset(&mut self) {
        self.waveform_buffer.clear();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let editor_open = self.params.editor_state.is_open();

        for channel_samples in buffer.iter_samples() {
            let gain = self.params.gain.smoothed.next();
            let mut first_sample = 0.0_f32;

            for (i, sample) in channel_samples.into_iter().enumerate() {
                *sample *= gain;
                if i == 0 {
                    first_sample = *sample;
                }
            }

            if editor_open {
                self.waveform_buffer.push(first_sample);
            }
        }

        ProcessStatus::Normal
    }

    fn deactivate(&mut self) {}
}

impl Vst3Plugin for Gain {
    // PERMANENT: changing this breaks all saved DAW projects using this plugin.
    // Generated from UUID EE0DEED6-D366-497F-B95F-66C2284C2C77
    const VST3_CLASS_ID: [u8; 16] = [
        0xEE, 0x0D, 0xEE, 0xD6, 0xD3, 0x66, 0x49, 0x7F,
        0xB9, 0x5F, 0x66, 0xC2, 0x28, 0x4C, 0x2C, 0x77,
    ];
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Tools];
}

impl ClapPlugin for Gain {
    const CLAP_ID: &'static str = "com.walkerware.quaker-waker";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("A simple gain plugin");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Stereo,
        ClapFeature::Mono,
        ClapFeature::Utility,
    ];
}

nih_export_clap!(Gain);
nih_export_vst3!(Gain);
