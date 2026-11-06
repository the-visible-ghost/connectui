use crate::ui::common::Props;
use iocraft::prelude::*;

#[component]
pub fn VpnContent(_hooks: Hooks, props: &Props) -> impl Into<AnyElement<'static>> {
    element! {
        View(flex_grow: 1.0, align_items: AlignItems::Center, justify_content: JustifyContent::Center) {
            Text(content: "Coming Soon ...", color: props.theme.primary)
        }
    }
}
