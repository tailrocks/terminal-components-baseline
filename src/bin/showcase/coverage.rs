pub const PICKER_COVERAGE: &[(&str, &str)] = &[
    ("widgets::picker::PickerStatus", "pickers"),
    ("widgets::picker::PickerStatus::Ready", "pickers"),
    ("widgets::picker::PickerStatus::Loading", "pickers"),
    ("widgets::picker::PickerStatus::Error", "pickers"),
    ("widgets::picker::Picker", "pickers"),
    ("widgets::picker::PickerAction", "pickers"),
    ("widgets::picker::PickerAction::QueryChanged", "pickers"),
    ("widgets::picker::PickerAction::Chosen", "pickers"),
    ("widgets::picker::PickerAction::ChosenMany", "pickers"),
    ("widgets::picker::PickerAction::Toggled", "pickers"),
    (
        "widgets::picker::PickerAction::SelectionConflict",
        "pickers",
    ),
    ("widgets::picker::PickerAction::ChosenAlt", "pickers"),
    ("widgets::picker::PickerAction::Submit", "pickers"),
    ("widgets::picker::PickerAction::Secondary", "pickers"),
    ("widgets::picker::PickerAction::NextScope", "pickers"),
    ("widgets::picker::PickerAction::PrevScope", "pickers"),
    ("widgets::picker::PickerAction::Cancelled", "pickers"),
    ("widgets::picker::PickerAction::Back", "pickers"),
    ("widgets::picker::PickerKeyProfile", "pickers"),
    ("widgets::picker::PickerKeyProfile::Desktop", "pickers"),
    ("widgets::picker::PickerKeyProfile::Vim", "pickers"),
    ("widgets::picker::Picker::new", "pickers"),
    ("widgets::picker::Picker::multi_select", "pickers"),
    ("widgets::picker::Picker::key_profile", "pickers"),
    ("widgets::picker::Picker::nav", "pickers"),
    ("widgets::picker::Picker::delete_is_secondary", "pickers"),
    ("widgets::picker::Picker::set_searchable", "pickers"),
    ("widgets::picker::Picker::query", "pickers"),
    ("widgets::picker::Picker::width", "pickers"),
    ("widgets::picker::Picker::placement", "pickers"),
    ("widgets::picker::Picker::anchor", "pickers"),
    ("widgets::picker::Picker::fit", "pickers"),
    ("widgets::picker::Picker::backdrop", "pickers"),
    ("widgets::picker::Picker::hints", "pickers"),
    ("widgets::picker::Picker::max_rows", "pickers"),
    ("widgets::picker::Picker::placeholder", "pickers"),
    ("widgets::picker::Picker::empty", "pickers"),
    ("widgets::picker::Picker::empty_text", "pickers"),
    ("widgets::picker::Picker::dismiss", "pickers"),
    ("widgets::picker::Picker::cursor_style", "pickers"),
    ("widgets::picker::Picker::set_items", "pickers"),
    ("widgets::picker::Picker::replace_items", "pickers"),
    ("widgets::picker::Picker::set_visible_items", "pickers"),
    ("widgets::picker::Picker::set_cursor", "pickers"),
];

pub const PICKER_ALLOWLIST: &[(&str, &str)] = &[
    (
        "widgets::picker::DEFAULT_GLYPH",
        "Every showcase row sets its glyph to show the list style explicitly, so the default glyph is not visible in this page.",
    ),
    (
        "widgets::picker::PickerCmd",
        "The page demonstrates toggle behavior through picker events and configured profiles; it does not expose the widget command type as a page action.",
    ),
    (
        "widgets::picker::PickerCmd::Toggle",
        "The page demonstrates toggle behavior through picker events and configured profiles; it does not expose the widget command variant as a page action.",
    ),
    (
        "widgets::picker::PickerKeyProfile::install",
        "The page configures each picker through its builder and intentionally does not change the thread-local default profile.",
    ),
    (
        "widgets::picker::PickerKeyProfile::current",
        "The page configures each picker through its builder and intentionally does not read the thread-local default profile.",
    ),
    (
        "widgets::picker::Picker::MAX_ROWS",
        "The page exposes row limits through the configured builder and does not refer to the widget's default constant directly.",
    ),
    (
        "widgets::picker::Picker::WIDTH",
        "The page exposes widths through the configured builder and does not refer to the widget's default constant directly.",
    ),
    (
        "widgets::picker::Picker::choose_hint",
        "The page demonstrates choice labels through its configured hint lines rather than this per-picker label builder.",
    ),
    (
        "widgets::picker::Picker::keymap",
        "The page demonstrates the supported Desktop and Vim profiles; custom keymaps are not a page configuration option.",
    ),
    (
        "widgets::picker::Picker::toggle_binding",
        "The page demonstrates the supported Desktop and Vim profiles; custom toggle bindings are not a page configuration option.",
    ),
    (
        "widgets::picker::Picker::searchable",
        "The page uses the explicit searchable state setter for its nonsearchable examples and does not need the builder form.",
    ),
    (
        "widgets::picker::Picker::is_multi",
        "The page presents multi-select behavior through the picker layer and its resulting actions, rather than reading widget mode directly.",
    ),
    (
        "widgets::picker::Picker::checked",
        "The page presents multi-select behavior through the picker layer and its resulting actions, rather than reading the checked-key buffer directly.",
    ),
    (
        "widgets::picker::Picker::toggle_checked",
        "The page presents multi-select behavior through delivered input events rather than manipulating selection state directly.",
    ),
    (
        "widgets::picker::Picker::reset_selection",
        "Picker layers in the showcase are ephemeral and have no user-facing reset action; the widget state test exercises the reset operation directly.",
    ),
    (
        "widgets::picker::Picker::caret",
        "The page demonstrates filtering through terminal input and does not inspect the edit caret directly.",
    ),
    (
        "widgets::picker::Picker::set_query",
        "The page demonstrates filtering through terminal input and does not set the query programmatically.",
    ),
    (
        "widgets::picker::Picker::is_searchable",
        "The page presents search behavior through visible picker interaction and does not inspect the internal mode flag.",
    ),
    (
        "widgets::picker::Picker::current",
        "The page presents row selection through the picker layer and does not inspect the selected item directly.",
    ),
    (
        "widgets::picker::Picker::current_key",
        "The page presents row selection through the picker layer and does not inspect the selected key directly.",
    ),
    (
        "widgets::picker::Picker::eligible",
        "The page presents disabled-row behavior through visible rows and does not query eligible indices directly.",
    ),
    (
        "widgets::picker::Picker::item_for_key",
        "The page presents item behavior through visible rows and does not query the backing item map directly.",
    ),
    (
        "widgets::picker::Picker::height",
        "The page delegates sizing to its layer and layout system rather than calling the widget height helper directly.",
    ),
    (
        "widgets::picker::Picker::on_click_outside",
        "The page demonstrates dismissal through the layer interaction and does not invoke its low-level outside-click handler.",
    ),
    (
        "widgets::picker::Picker::on_key",
        "The page demonstrates keyboard behavior through terminal events and does not call the widget event handler directly.",
    ),
    (
        "widgets::picker::Picker::on_click",
        "The page demonstrates pointer behavior through terminal events and does not call the widget event handler directly.",
    ),
    (
        "widgets::picker::Picker::on_paste",
        "The page demonstrates filtering through ordinary terminal input and has no paste-specific picker scenario.",
    ),
    (
        "widgets::picker::Picker::row_id",
        "The page delegates row identity and pointer routing to the picker layer rather than calling the row-id helper directly.",
    ),
    (
        "widgets::picker::Picker::draw",
        "The page composes pickers through layers and does not draw the widget directly into a buffer.",
    ),
    (
        "widgets::picker::Picker::render",
        "The page composes pickers through layers and does not call the widget's direct render entry point.",
    ),
];
