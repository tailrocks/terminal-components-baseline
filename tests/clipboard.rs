use std::sync::Arc;

use junie_tui::shell::{Clipboard, Copied, NativeClipboard};

#[derive(Debug)]
struct Sink(bool);

impl NativeClipboard for Sink {
    fn write(&self, _text: &str) -> bool {
        self.0
    }
}

const TEXT: &str = "the settlement retry loop so failed batches";

#[test]
fn a_native_write_overrides_an_osc52_refusal() {
    let chars = TEXT.chars().count();
    let too_large = || Clipboard::new().limit(8);
    let unsupported = || Clipboard::new().osc52(false);

    assert_eq!(
        too_large().native(Arc::new(Sink(true))).copy(TEXT),
        Copied::Copied { chars }
    );
    assert_eq!(
        too_large().native(Arc::new(Sink(false))).copy(TEXT),
        Copied::TooLarge {
            encoded: TEXT.len().div_ceil(3) * 4,
            limit: 8
        }
    );
    assert_eq!(
        unsupported().native(Arc::new(Sink(true))).copy(TEXT),
        Copied::Copied { chars }
    );
    assert_eq!(
        unsupported().native(Arc::new(Sink(false))).copy(TEXT),
        Copied::Unsupported
    );
}
