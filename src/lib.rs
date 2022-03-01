use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::error;
use std::fs;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Project {
    pub time: usize, // TODO this should not be pub if possible.
    sequence: Vec<Waveform>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Parameters {
    #[serde(default)]
    time: usize,
}

impl Default for Parameters {
    fn default() -> Self {
        Parameters { time: 1 }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Waveform {
    Sine(Parameters),
    Saw(Parameters),
    Square(Parameters),
    Noise(Parameters),
    NoiseSimplex(Parameters),
}

impl Default for Project {
    fn default() -> Self {
        let default_project = include_str!("default_project.json");
        serde_json::from_str::<Self>(default_project).unwrap()
    }
}

impl TryFrom<String> for Project {
    type Error = Box<dyn error::Error>;

    fn try_from(path: String) -> Result<Self, Self::Error> {
        let project_json = fs::read_to_string(path)?;
        let result = serde_json::from_str::<Self>(project_json.as_str())?;
        Ok(result)
    }
}

impl Project {
    pub fn sequence(&self) -> &[Waveform] {
        return &self.sequence;
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_waveform_parameters_serde() {
        let expected = Waveform::Sine(Parameters::default());
        let expected_as_json = serde_json::to_string(&expected).unwrap();
        let actual = serde_json::from_str::<Waveform>(&expected_as_json).unwrap();
        assert_eq!(expected, actual);
    }
}
