use nih_plug::prelude::*;
use super::ModulableParamHandle;

#[derive(Enum, PartialEq, Eq)]
enum DistortionType {
    SoftClip,
    DownSample,
    HardClip
}

#[derive(Params)]
pub struct DistortionParams {
    #[id = "drive"] pub drive: ModulableParamHandle<FloatParam>,
    #[id = "mix"]   pub   mix: ModulableParamHandle<FloatParam>,
    #[id = "type"]  kind: EnumParam<DistortionType>,
}