use iocraft::prelude::*;

#[derive(Default, Props)]
pub struct ThematicBreakProps {}

#[component]
pub fn ThematicBreak(_props: &ThematicBreakProps, _hooks: Hooks) -> impl Into<AnyElement<'static>> {
    element! {
        View(margin_bottom: 1) {
            Text(content: "───────────────────────────────".to_string(), color: crate::theme::DARK_GREY)
        }
    }
}
