//! Rune's own combobox scenes; there is no Storybook story to match.

use rune::El;
use rune_kit::{Combobox, ComboboxOption, Kit};

use crate::G;

pub(super) fn searchable(s: &G, k: &Kit, layer: &mut Vec<El>) -> El {
    let options: Vec<_> = (0..200).map(|i| ComboboxOption::new(format!("model-{i}"), format!("Model {i:03}"))).collect();
    Combobox::new("model", &s.combo, &options, |s: &mut G| &mut s.combo)
        .placeholder("Choose model").current(s.pick.get("model").map(String::as_str))
        .on_choose(|s, id, _| { s.pick.insert("model".into(), id.into()); })
        .build(k, layer)
}
