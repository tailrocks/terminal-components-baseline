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
            bytes: TEXT.len(),
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

#[derive(Debug, Default)]
struct Counting(std::sync::atomic::AtomicUsize);

impl NativeClipboard for Counting {
    fn write(&self, _text: &str) -> bool {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        true
    }
}

fn queued() -> Vec<String> {
    junie_tui::shell::clipboard::take_output()
}

#[test]
fn the_limit_counts_decoded_bytes_like_the_pane_policy() {
    assert_eq!(Clipboard::DEFAULT_LIMIT, 100_000);
    let at_limit = "x".repeat(100_000);
    let over = "x".repeat(100_001);
    let clipboard = Clipboard::new();

    assert_eq!(
        clipboard.copy(&"x".repeat(80_000)),
        Copied::Copied { chars: 80_000 },
        "80,000 decoded bytes are 106,668 encoded; the pane policy accepts them"
    );
    assert_eq!(clipboard.copy(&at_limit), Copied::Copied { chars: 100_000 });
    let refused = clipboard.copy(&over);
    assert!(
        matches!(refused, Copied::TooLarge { limit: 100_000, .. }),
        "{refused:?}"
    );
    assert_eq!(
        Clipboard::report(&refused, &over, None).0,
        "Not copied · 100001 bytes exceed the terminal's 100000 byte OSC 52 limit"
    );
    queued();
}

#[test]
fn osc52_is_queued_only_when_the_native_write_did_not_land() {
    queued();
    let native = Arc::new(Counting::default());
    let landed = Clipboard::new().native(native.clone()).copy(TEXT);
    assert_eq!(
        landed,
        Copied::Copied {
            chars: TEXT.chars().count()
        }
    );
    assert_eq!(native.0.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(
        queued(),
        Vec::<String>::new(),
        "no OSC 52 after a native write"
    );

    let missed = Clipboard::new().native(Arc::new(Sink(false))).copy(TEXT);
    assert_eq!(
        missed,
        Copied::Requested {
            chars: TEXT.chars().count()
        }
    );
    assert_eq!(
        queued().len(),
        1,
        "a failed native write falls back to OSC 52"
    );

    Clipboard::new().copy(TEXT);
    assert_eq!(queued().len(), 1, "no native sink: OSC 52 only");
}
