//! Model replies (and short authored strings) as real formatting: shared::md's tree turned into elements.
//! Text goes in as text nodes, never as HTML, so nothing a model writes can become markup or a link.
use leptos::prelude::*;
use shared::md::{blocks, inline, Block, Inline};

fn runs(v: Vec<Inline>) -> AnyView {
    v.into_iter().map(|i| match i {
        Inline::Text(t) => t.into_any(),
        Inline::Bold(x) => view! { <strong>{runs(x)}</strong> }.into_any(),
        Inline::Italic(x) => view! { <em>{runs(x)}</em> }.into_any(),
        Inline::Code(c) => view! { <code>{c}</code> }.into_any(),
        Inline::Break => view! { <br/> }.into_any(),
    }).collect_view().into_any()
}

/// One line or run: bold, italic, code and line breaks only (for list items, nudges, a guide step).
pub fn line(s: &str) -> AnyView { runs(inline(s)) }

/// A whole reply: paragraphs, lists, headings (bold lines) and code blocks. Put it in a block element.
pub fn reply(s: &str) -> AnyView {
    blocks(s).into_iter().map(|b| match b {
        Block::Para(x) => view! { <p>{runs(x)}</p> }.into_any(),
        Block::Heading(x) => view! { <p class="md-h"><strong>{runs(x)}</strong></p> }.into_any(),
        Block::Bullets(items) => view! { <ul>{items.into_iter().map(|x| view! { <li>{runs(x)}</li> }).collect_view()}</ul> }.into_any(),
        Block::Numbered(start, items) => view! { <ol start=start.to_string()>{items.into_iter().map(|x| view! { <li>{runs(x)}</li> }).collect_view()}</ol> }.into_any(),
        Block::Code(c) => view! { <pre><code>{c}</code></pre> }.into_any(),
    }).collect_view().into_any()
}
