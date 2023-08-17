use crate::component::positioner::Position;
use crate::translation::use_translation;
use core_protocol::id::LanguageId;
use stylist::yew::styled_component;
use web_sys::TransitionEvent;
use yew::{classes, hook, html, use_state, Callback, Html, Properties};

#[derive(Copy, Clone, PartialEq, Properties)]
pub struct InstructionsProps {
    pub position: Position,
    pub primary: Instruction,
    pub secondary: Instruction,
}

pub type Instruction = Option<fn(LanguageId) -> &'static str>;

#[styled_component(Instructions)]
pub fn instructions(props: &InstructionsProps) -> Html {
    let div_style = css!(
        r#"
        pointer-events: none;
        user-select: none;
        color: white;
        "#
    );

    let fade = css!(
        r#"
        opacity: 0.4;
        transition: opacity 0.5s;
        "#
    );

    let active = css!(
        r#"
        opacity: 1.0;
        "#
    );

    let t = use_translation();

    #[allow(clippy::type_complexity)]
    #[hook]
    fn use_instruction(
        instruction: Instruction,
    ) -> (
        fn(LanguageId) -> &'static str,
        Option<Callback<TransitionEvent>>,
    ) {
        // Stores the instructions we are transitioning *from*
        // and whether the transition is running.
        let current = use_state::<Instruction, _>(|| None);
        let fading = use_state(|| false);
        let fade = {
            let current = current.clone();
            let fading = fading.clone();
            Callback::from(move |_| {
                current.set(None);
                fading.set(false);
            })
        };

        if instruction != *current {
            if let Some(current) = *current {
                if !*fading {
                    fading.set(true);
                }
                (current, Some(fade))
            } else {
                current.set(instruction);
                (instruction.unwrap_or(|_| ""), None)
            }
        } else if let Some(new) = instruction {
            (new, None)
        } else {
            (|_| "", Some(fade))
        }
    }

    let (primary, on_primary_transitionend) = use_instruction(props.primary);
    let (secondary, on_secondary_transitionend) = use_instruction(props.secondary);

    html! {
        <div id="instructions" class={div_style} style={props.position.to_string()}>
            <h2
                style={"font-size: 1.5rem; margin-top: 0.5rem; margin-bottom: 0;"}
                class={classes!(fade.clone(), on_primary_transitionend.is_none().then(|| active.clone()))}
                ontransitionend={on_primary_transitionend}
            >
                {primary(t)}
            </h2>
            <p
                style={"font-size: 1.25rem; margin-top: 0.5rem;"}
                class={classes!(fade, on_secondary_transitionend.is_none().then_some(active))}
                ontransitionend={on_secondary_transitionend}
            >
                {secondary(t)}
            </p>
        </div>
    }
}
