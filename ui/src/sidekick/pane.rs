//! Sidekick's pane: the same component in two places. In a coaching session it is the Sidekick column beside the
//! practice app pane; on course pages it is the sidekick that slides in from the right edge (sidekick.rs draws its
//! tab). Coach (stages and a hint ladder) or Ask Sidekick (straight answers), one remembered mode for both.
//! Outside a session Coach works on the lesson or exercise on screen, and the screen helper buttons give way
//! to "Practice in the app", which starts a session.
use super::{get, put, reduced_motion, Side, Starter};
use crate::{android, api, ctx, md, msg_view, Icon, Page, Pmode, PracticeError, ScreenLine, ScreenLog};
use leptos::{prelude::*, task::spawn_local};
use serde_json::json;
use shared::*;

/// Example chips and composer ghost lines for an empty Ask pane, by page. `go:` chips act locally.
pub fn ask_examples(p: &Page) -> &'static [&'static str] {
    match p {
        Page::Lesson(_) => &["What's on this lesson?", "Quiz me", "go:Where was I?"],
        Page::Course => &["go:Where was I?", "What's in this course?", "What should I learn first?"],
        Page::Session(_) => &["What's in this session?", "go:Where was I?"],
        Page::Check(_) | Page::Final => &["How do checks work?", "go:Where was I?"],
        Page::Exercise(_) => &["How do I start this exercise?", "What do the criteria mean?"],
        Page::Settings => &["Which AI provider should I pick?", "What's the practice app?"],
        Page::Certificate => &["go:What's left for my certificate?", "How is the certificate issued?"],
    }
}

/// What a chip sends: "Quiz me" asks for a proper little quiz.
pub fn chip_question(label: &str) -> String {
    match label {
        "Quiz me" => "Quiz me on this lesson: three short questions, one at a time, and tell me how I did.".into(),
        l => l.to_string(),
    }
}

/// The lesson or exercise Coach works on outside a session: the page on screen.
fn page_target(p: &Page) -> Option<(String, String)> {
    match p {
        Page::Lesson(id) => Some(("lesson".into(), id.clone())),
        Page::Exercise(id) => Some(("exercise".into(), id.clone())),
        _ => None,
    }
}

/// The ghost placeholder: example questions typed one letter at a time while the composer is empty
/// (none under reduced motion).
fn ghost(examples: Signal<Vec<String>>, active: Signal<bool>) -> RwSignal<String> {
    let g = RwSignal::new(String::new());
    let step = StoredValue::new((0usize, 0usize));
    let h = set_interval_with_handle(move || {
        let ex = examples.get_untracked();
        if !active.get_untracked() || reduced_motion() || ex.is_empty() { if !g.get_untracked().is_empty() { g.set(String::new()) } return; }
        let (mut i, mut n) = step.get_value();
        let line: Vec<char> = ex[i % ex.len()].chars().collect();
        n += 1;
        if n > line.len() + 25 { i += 1; n = 0; }
        step.set_value((i, n));
        let line: Vec<char> = ex[i % ex.len()].chars().collect();
        let mut s: String = line.iter().take(n).collect();
        if n <= line.len() { s.push('▍') }
        g.set(s);
    }, std::time::Duration::from_millis(55));
    on_cleanup(move || { if let Ok(h) = h { h.clear() } });
    g
}

#[component]
pub fn SidekickPane(
    /// The sidekick on course pages (not the session column).
    #[prop(optional)] side: bool,
    /// Extra content above the composer (the sidekick's Where to?).
    #[prop(optional)] extra: Option<ViewFn>,
) -> impl IntoView {
    let c = ctx();
    let sk = use_context::<Side>().expect("sidekick side");
    let ask_mode = sk.ask;
    let coach_state = if side { sk.coach } else { c.coach };
    let draft = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let info = RwSignal::new(None::<String>);
    let ask_msgs = RwSignal::new(Vec::<Msg>::new());
    let body = NodeRef::<leptos::html::Div>::new();
    let feed = NodeRef::<leptos::html::Div>::new();
    let input = NodeRef::<leptos::html::Textarea>::new();
    // The log reads like a chat app: anchored to the bottom, above the composer. It sticks to the newest
    // line while the learner is at (or within 40 px of) the bottom, and a resize observer on the feed keeps
    // it there through anything that grows later (a reply, streamed screen-helper steps, a re-render).
    // Scrolled up to read, it stays put and the "New messages" pill offers the way back down.
    let stick = StoredValue::new(true);
    let unseen = RwSignal::new(false);
    let fed = StoredValue::new(0i32);
    let to_end = move || if let Some(el) = body.get_untracked() { el.set_scroll_top(el.scroll_height()) };
    // the learner's own move (sending, a new conversation, the pill): go to the newest and stay there
    let pin = move || { stick.set_value(true); unseen.set(false); to_end(); request_animation_frame(to_end); };
    let on_scroll = move |_| if let Some(el) = body.get_untracked() {
        let at = el.scroll_height() - el.scroll_top() - el.client_height() <= 40;
        stick.set_value(at);
        if at && unseen.get_untracked() { unseen.set(false) }
    };
    let observer = StoredValue::new_local(None::<(web_sys::ResizeObserver, wasm_bindgen::closure::Closure<dyn Fn()>)>);
    feed.on_load(move |el| {
        use wasm_bindgen::JsCast;
        let el: web_sys::HtmlElement = el.into();
        let follow = { let el = el.clone(); move || {
            let h = el.offset_height();
            let grew = h > fed.get_value();
            fed.set_value(h);
            if stick.get_value() { to_end() } else if grew && !unseen.get_untracked() { unseen.set(true) }
        } };
        let cb = wasm_bindgen::closure::Closure::<dyn Fn()>::new(follow);
        let Ok(ro) = web_sys::ResizeObserver::new(cb.as_ref().unchecked_ref()) else { return };
        ro.observe(&el);
        // the log's own box changes too (the composer grows, the window resizes)
        if let Some(log) = el.parent_element() { ro.observe(&log) }
        observer.set_value(Some((ro, cb)));
    });
    on_cleanup(move || if let Some((ro, _cb)) = observer.try_update_value(Option::take).flatten() { ro.disconnect() });
    // what Coach works on: the session's lesson or exercise, or (sidekick) the page on screen
    let target = move || -> Option<(String, String)> {
        if side { page_target(&c.page.get_untracked()) } else { c.session.get_untracked().map(|(k, id, _)| (k, id)) }
    };
    let lesson_id = move || target().and_then(|(k, id)| (k == "lesson").then_some(id));
    let coach = move |message: Option<String>, hint: bool| {
        let Some((kind, id)) = target() else { return };
        if message.is_some() || hint { pin() }
        let args = json!({ "course_id": c.course.get_untracked(), "kind": kind, "id": id, "message": message, "hint": hint });
        busy.set(true);
        spawn_local(async move {
            match api::call::<CoachState>("coach_turn", args).await {
                Ok(st) => { coach_state.set(Some(st)); error.set(None) }
                Err(e) => error.set(Some(e)),
            }
            busy.set(false);
        });
    };
    // the sidekick's Coach follows the page: load that lesson's coaching (no model call until asked)
    let loaded_for = StoredValue::new(None::<(String, String)>);
    let load_coach = move |then: Option<Starter>| {
        let Some((kind, id)) = target() else { coach_state.set(None); loaded_for.set_value(None); return };
        let course = c.course.get_untracked();
        let key = (kind.clone(), id.clone());
        spawn_local(async move {
            if let Ok(st) = api::call::<CoachState>("get_coach", json!({ "course_id": course, "kind": kind, "id": id })).await {
                let fresh = st.messages.is_empty();
                coach_state.set(Some(st));
                loaded_for.set_value(Some(key));
                match then {
                    Some(Starter::Coach) if fresh => coach(None, false),
                    Some(Starter::Coach) => coach(Some("I'm stuck on this stage. Can you help?".into()), false),
                    _ => {}
                }
                pin();
            }
        });
    };
    if side {
        Effect::new(move |_| {
            let p = c.page.get();
            if ask_mode.get() { return; }
            let t = page_target(&p);
            if t != loaded_for.get_value() { load_coach(None) }
        });
    }
    // Sidekick opens a session, and each new stage, on its own (the sidekick waits to be asked)
    Effect::new(move |prev: Option<Option<usize>>| {
        let st = coach_state.get();
        let cur = st.as_ref().map(|x| x.stage);
        if let Some(st) = &st {
            let stage_changed = matches!(prev, Some(Some(p)) if Some(p) != cur);
            if ((st.messages.is_empty() && !side) || stage_changed) && !busy.get_untracked() { coach(None, false) }
        }
        cur
    });
    Effect::new(move |_| {
        if !ask_mode.get() { return; }
        c.page.track();
        let args = json!({ "course_id": c.course.get_untracked(), "lesson_id": lesson_id() });
        spawn_local(async move { ask_msgs.set(api::call::<Vec<Msg>>("get_messages", args).await.unwrap_or_default()); pin(); });
    });
    // "look" / "act": the compose box sends the next message to the screen helper instead of the coach
    let screen_mode = sk.screen_mode;
    let run_screen = move |mode: &'static str, task: Option<String>| {
        let Some((kind, id, _)) = c.session.get_untracked() else { return };
        if let Some(t) = &task { c.screen.update(|v| v.push(ScreenLine { kind: "you".into(), text: t.clone() })) }
        c.screen_busy.set(true);
        pin();
        let args = json!({ "course_id": c.course.get_untracked(), "kind": kind, "id": id, "mode": mode, "task": task });
        spawn_local(async move {
            if let Err(e) = api::call::<()>("screen_agent_run", args).await {
                c.screen_busy.set(false);
                c.screen.update(|v| v.push(ScreenLine { kind: "error".into(), text: e }));
            }
        });
    };
    let answer = move |allow: bool| {
        c.screen_ask.set(None);
        spawn_local(async move { let _ = api::call::<()>("screen_agent_answer", json!({ "allow": allow })).await; });
    };
    let in_exercise = move || c.session.get().is_some_and(|(k, _, _)| k == "exercise");
    let ask = move |q: String| {
        ask_msgs.update(|m| m.push(Msg { role: "user".into(), content: q.clone() }));
        pin();
        busy.set(true);
        let args = json!({ "course_id": c.course.get_untracked(), "lesson_id": lesson_id(), "question": q });
        spawn_local(async move {
            match api::call::<Msg>("ask_tutor", args).await {
                Ok(r) => { ask_msgs.update(|m| m.push(r)); error.set(None) }
                Err(e) => error.set(Some(e)),
            }
            busy.set(false);
        });
    };
    let send = move || {
        let q = draft.get_untracked().trim().to_string();
        if q.is_empty() || busy.get_untracked() { return; }
        draft.set(String::new());
        if let Some(mode) = screen_mode.get_untracked().filter(|_| !side) { return run_screen(mode, Some(q)); }
        if !ask_mode.get_untracked() { return coach(Some(q), false); }
        ask(q)
    };
    // something outside asked for a start: a question, coaching, or words in the composer
    Effect::new(move |_| {
        let Some(s) = sk.starter.get() else { return };
        sk.starter.set(None);
        match s {
            Starter::Ask(q) => { ask_mode.set(true); ask(q) }
            Starter::Coach => { ask_mode.set(false); if side { load_coach(Some(Starter::Coach)) } else { coach(Some("I'm stuck on this stage. Can you help?".into()), false) } }
            Starter::Draft(t) => {
                draft.set(t);
                request_animation_frame(move || if let Some(i) = input.get_untracked() { let _ = i.focus(); });
            }
        }
    });
    let go_stage = move |n: usize| {
        let Some((kind, id)) = target() else { return };
        let args = json!({ "course_id": c.course.get_untracked(), "kind": kind, "id": id, "stage": n });
        spawn_local(async move { if let Ok(st) = api::call::<CoachState>("set_coach_stage", args).await { coach_state.set(Some(st)) } });
    };
    let next_stage = move |_| {
        let (Some(st), Some((kind, id))) = (coach_state.get_untracked(), target()) else { return };
        if st.stage + 1 >= st.stages.len() { return; }
        let args = json!({ "course_id": c.course.get_untracked(), "kind": kind, "id": id, "stage": st.stage + 1 });
        spawn_local(async move { if let Ok(st) = api::call::<CoachState>("set_coach_stage", args).await { coach_state.set(Some(st)) } });
    };
    let open_session = use_context::<Callback<(String, String, String)>>();
    let practice = move |_| {
        let (Some(open), Some((kind, id))) = (open_session, target()) else { return };
        let label = c.outline.with_untracked(|o| o.as_ref().and_then(|cv| crate::sidekick::find_lesson(cv, &id).map(|l| format!("{} {}", l.number, l.title)))).unwrap_or_else(|| id.clone());
        open.run((kind, id, label));
    };
    let target_label = move || {
        let (kind, id) = target()?;
        if kind == "exercise" { return Some("this exercise".to_string()); }
        c.outline.with(|o| o.as_ref().and_then(|cv| crate::sidekick::find_lesson(cv, &id).map(|l| format!("{} {}", l.number, l.title))))
    };
    let pose = move || {
        let p = sk.pose.get();
        if busy.get() { "thinking".to_string() } else if p != "idle" { p } else if ask_mode.get() { "wave".into() } else { "pointing".into() }
    };
    let examples = Signal::derive(move || {
        let p = c.page.get();
        if ask_mode.get() { ask_examples(&p).iter().map(|e| e.trim_start_matches("go:").to_string()).collect() }
        else { vec![{concat!("Tell ", shared::companion_name!(), " what you did or where you're stuck…")}.to_string()] }
    });
    let ghost_on = Signal::derive(move || draft.with(String::is_empty) && ask_mode.get() && ask_msgs.with(Vec::is_empty));
    let ghost_text = ghost(examples, ghost_on);
    // the other mode is another conversation: open it at its newest line
    let set_mode = move |ask: bool| { pin(); ask_mode.set(ask); put("sidekick_pane_mode", if ask { "ask" } else { "coach" }) };
    let _ = get; // (storage helpers shared with sidekick.rs)
    view! {
        <aside class="sidekick-pane" class:side=side aria-label={concat!(shared::companion_name!())}>
            <header class="pane-head">
                <button class="pane-sidekick" data-help="side.sidekick" class:away=move || sk.away.get() on:click=move |_| sk.poke.run(()) aria-label={concat!(shared::companion_name!(), ": play a trick")}>
                    <span class=move || format!("sidekick-fx {}", sk.fx.get())>
                        <img src=move || format!("assets/mascot/{}.png", pose()) alt="" width="44" height="44"
                            on:error=move |ev| { let el: web_sys::HtmlImageElement = event_target(&ev); el.set_src("assets/icon.png"); }/>
                    </span>
                </button>
                <div class="pane-title"><strong>{concat!(shared::companion_name!())}</strong>
                    <span aria-live="polite">{move || {
                        let q = sk.quip.get();
                        if !q.is_empty() { q } else if busy.get() { "Thinking…".into() } else if ask_mode.get() { "Straight answers".into() } else { "Coaching you through it".into() }
                    }}</span></div>
                <div class="mode-switch" class:right=move || ask_mode.get() role="radiogroup" aria-label={concat!(shared::companion_name!(), " mode")}>
                    <button role="radio" data-help="coach.mode-coach" aria-checked=move || (!ask_mode.get()).to_string() class:on=move || !ask_mode.get()
                        on:click=move |_| set_mode(false)><Icon name="steps"/>"Coach"</button>
                    <button role="radio" data-help="coach.mode-ask" aria-checked=move || ask_mode.get().to_string() class:on=move || ask_mode.get()
                        on:click=move |_| set_mode(true)><Icon name="chat"/>{concat!("Ask ", shared::companion_name!())}</button>
                </div>
                {side.then(|| view! {
                    <button class="icon-btn small" data-help="side.collapse" on:click=move |_| c.chat_open.set(false) aria-label={concat!("Tuck ", shared::companion_name!(), " away")}><Icon name="right"/></button>
                })}
            </header>
            {move || (!ask_mode.get()).then(|| coach_state.get().map(|st| {
                let cur = st.stage;
                let total = st.stages.len();
                view! {
                <ol class="stages" aria-label="Session stages">
                    {(0..total).map(|i| view! {
                        <li><button data-help="coach.stage" class:here=i == cur class:done=i < cur aria-label=format!("Stage {}: {}", i + 1, st.stages[i])
                            on:click=move |_| go_stage(i)>{if i < cur { "✓".to_string() } else { (i + 1).to_string() }}</button></li>
                    }).collect_view()}
                </ol>
                <div class="stage-card">
                    <span class="meta">{format!("Stage {} of {}", st.stage + 1, st.stages.len())}</span>
                    <p>{st.stages.get(st.stage).cloned().unwrap_or_default()}</p>
                </div>
            }}))}
            <div class="pane-log">
            <div class="pane-body" node_ref=body aria-live="polite" on:scroll=on_scroll>
            <div class="pane-feed" node_ref=feed>
                {(!side).then(|| view! { <PracticeError/> })}
                {move || if ask_mode.get() {
                    let m = ask_msgs.get();
                    if m.is_empty() {
                        let p = c.page.get();
                        view! {
                            <p class="empty">{if side { "Ask me anything about the course. I answer from the lessons and say when a question goes beyond them." }
                                else { "Ask anything about this lesson: approaches, settings, edge cases. No hint ladder here, just straight answers." }}</p>
                            <div class="sidekick-chips ask-chips">
                                {ask_examples(&p).iter().map(|e| {
                                    let go = e.starts_with("go:");
                                    let label = e.trim_start_matches("go:");
                                    view! { <button class="chip" class:go=go on:click=move |_| if go { sk.go_next.run(()) } else { ask(chip_question(label)) }>{label}</button> }
                                }).collect_view()}
                            </div>
                        }.into_any()
                    } else { m.into_iter().map(msg_view).collect_view().into_any() }
                } else {
                    let msgs = coach_state.get().map(|st| st.messages).unwrap_or_default();
                    if side && msgs.is_empty() {
                        match target_label() {
                            Some(l) => view! {
                                <p class="empty">"I'll take it one stage at a time, with hints when you want them."</p>
                                <div class="sidekick-chips"><button class="chip coach-start" on:click=move |_| load_coach(Some(Starter::Coach)) disabled=move || busy.get()>{format!("Coach me through {l}")}</button></div>
                            }.into_any(),
                            None => view! {
                                <p class="empty">"Open a lesson or an exercise and I'll coach you through it, stage by stage."</p>
                                <div class="sidekick-chips"><button class="chip go" on:click=move |_| sk.go_next.run(())>"Take me to my next lesson"</button></div>
                            }.into_any(),
                        }
                    } else { msgs.into_iter().map(msg_view).collect_view().into_any() }
                }}
                {move || busy.get().then(|| view! { <p class="msg assistant typing" aria-label={concat!(shared::companion_name!(), " is replying")}><span></span><span></span><span></span></p> })}
                {move || info.get().map(|m| view! { <p class="notice">{m}</p> })}
                {move || error.get().map(|e| view! { <p class="notice error" role="alert">{md::line(&e)}</p> })}
                {(!side).then(|| view! { <ScreenLog answer/> })}
            </div>
            </div>
                <button class="pane-new" class:on=move || unseen.get() aria-hidden=move || (!unseen.get()).to_string() tabindex=move || if unseen.get() { "0" } else { "-1" }
                    on:click=move |_| pin()>"New messages"<Icon name="down"/></button>
            </div>
            {extra.map(|f| f.run())}
            <div class="pane-tools">
                {move || (!ask_mode.get() && coach_state.with(Option::is_some)).then(|| view! {
                    <button class="btn small" data-help="coach.hint" on:click=move |_| coach(None, true) disabled=move || busy.get()>
                        {move || match coach_state.get().map(|s| s.hints_used).unwrap_or(0) { 0 => "Show a hint".to_string(), n => format!("Another hint ({n} used)") }}
                    </button>
                    <button class="btn small" data-help="coach.next-stage" on:click=next_stage
                        disabled=move || coach_state.get().map(|s| s.stage + 1 >= s.stages.len()).unwrap_or(true)>"Next stage"<Icon name="right"/></button>
                })}
                {move || if side {
                    (target().is_some() && !android() && c.practice.get() == Pmode::Off).then(|| view! {
                        <button class="btn small" data-help="lesson.practice" on:click=practice>"Practice in the app"</button>
                    }).into_any()
                } else if c.screen_busy.get() {
                    view! { <button class="btn small" data-help="coach.stop" on:click=move |_| spawn_local(async move { let _ = api::call::<()>("screen_agent_stop", json!({})).await; })>{concat!("Stop ", shared::companion_name!())}</button> }.into_any()
                } else { view! {
                    <button class="btn small" data-help="coach.look" class:hidden=android class:on=move || screen_mode.get() == Some("look")
                        on:click=move |_| screen_mode.update(|m| *m = if *m == Some("look") { None } else { Some("look") })>"Look at my screen"</button>
                    <button class="btn small" data-help="coach.act" class:hidden=android class:on=move || screen_mode.get() == Some("act")
                        on:click=move |_| screen_mode.update(|m| *m = if *m == Some("act") { None } else { Some("act") })>"Do it for me"</button>
                    <button class="btn small" data-help="coach.check-build" class:hidden=move || android() || !in_exercise() on:click=move |_| run_screen("check", None)>"Check my build"</button>
                }.into_any() }}
            </div>
            <div class="pane-compose" data-help="coach.send">
                <textarea rows="2" node_ref=input aria-label={concat!("Message ", shared::companion_name!())} prop:value=move || draft.get() on:input=move |ev| draft.set(event_target_value(&ev))
                    on:keydown=move |ev| if ev.key() == "Enter" && !ev.shift_key() { ev.prevent_default(); send() }
                    placeholder=move || {
                        let g = ghost_text.get();
                        if !g.is_empty() { return g; }
                        match screen_mode.get().filter(|_| !side) {
                            Some("look") => "Ask about your screen, e.g. where do I add a tag?",
                            Some(_) => {concat!("Tell ", shared::companion_name!(), " what to do in the app, e.g. add a tag called Weekly")},
                            None if ask_mode.get() => "Ask a question…",
                            None => {concat!("Tell ", shared::companion_name!(), " what you did or where you're stuck…")},
                        }.to_string()
                    }></textarea>
                <button class="btn primary" on:click=move |_| send() disabled=move || busy.get() || draft.with(|d| d.trim().is_empty())>"Send"</button>
            </div>
        </aside>
    }
}
