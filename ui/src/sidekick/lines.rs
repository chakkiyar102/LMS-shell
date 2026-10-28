//! What Sidekick says when he isn't nudging: greetings, cheers, tap reactions, flight calls. Every
//! context has at least eight lines; `Picker` never says the same line twice in a row. `{name}` is the
//! learner's first name (lines that need it are skipped when there is none), `{n}` a number.
//! House rules for anyone adding lines: no em-dashes, no sarcasm about mistakes, nothing that needs a
//! pop-culture footnote, and never a promise about a feature the app doesn't have.

pub const LINES: &[(&str, &[&str])] = &[
    ("morning", &[
        "Morning, {name}. Fresh brain, fresh lesson.",
        "Good morning! I've been charging all night for this.",
        "Morning! Coffee for you, a full battery for me.",
        "Hi {name}. Mornings are great for learning. Also for toast.",
        "Good morning. Your next lesson missed you.",
        "Morning! Let's get one small thing done before the day gets busy.",
        "Hello, early bird. I saved you the best seat.",
        "Good morning, {name}. Ready when you are.",
    ]),
    ("afternoon", &[
        "Afternoon, {name}. Good time for a quick lesson.",
        "Hi again! The afternoon slump has no chance against us.",
        "Good afternoon. I'll be right here if you need me.",
        "Hello! Learning on a lunch break? Bold. I like it.",
        "Afternoon! One lesson now, a smug feeling all evening.",
        "Hey {name}. Let's make this afternoon count a little.",
        "Good afternoon. My gears are warmed up. Yours?",
        "Afternoon check-in: you showed up. That's the hard part.",
    ]),
    ("evening", &[
        "Evening, {name}. Let's learn something small, then go relax.",
        "Good evening! Perfect time for a calm lesson.",
        "Winding down the day with a lesson? Very wise.",
        "Evening! I dimmed my lights a little, just for the mood.",
        "Hello, {name}. One lesson, then feet up.",
        "Good evening. Quiet hours are good thinking hours.",
        "Evening! Let's pick up where you left off.",
        "The day's nearly done and you're still curious. Nice.",
    ]),
    ("late_night", &[
        "It's late, {name}. Learn a little, then rest. Brains file things away while you sleep.",
        "Night owl mode! I'll keep my lights low.",
        "Still up? Let's keep it short and sweet.",
        "Late night study session. I brought my softest beeps.",
        "Hi, night owl. A small lesson now is plenty.",
        "It's quiet out there. Good time to focus, and a good time to sleep soon.",
        "Up late? I run on batteries, so I'm fine, but you deserve a pillow soon.",
        "Psst. One lesson, then bed. Deal?",
    ]),
    ("back_days", &[
        "Welcome back, {name}! I kept your spot warm.",
        "You're back! I counted the days. Counting is my hobby.",
        "Good to see you again. Let's ease back in.",
        "Welcome back! No catching up needed, just the next small step.",
        "There you are! Your progress waited patiently. So did I.",
        "A short break is fine. Picking it back up is the win.",
        "Hello again! Let's start with something easy.",
        "{name}! Good timing. I was just polishing my badge.",
    ]),
    ("back_long", &[
        "{name}! It's been a while. Really glad you're here.",
        "Welcome back! Everything you did before is still saved.",
        "Long time no see! I dusted off the lessons for you.",
        "You came back. That matters more than how long it took.",
        "Welcome back. Let's take it one lesson at a time.",
        "It's been a bit! I'll point you to where you left off.",
        "Hello, old friend. Fancy a gentle restart?",
        "You're back! I may have practised my wave the whole time.",
    ]),
    ("quiz_pass", &[
        "Passed! I knew you had it.",
        "Nice work, {name}. That's a pass.",
        "Look at you go. Check passed!",
        "Passed! My circuits are doing a tiny happy dance.",
        "That's a pass. Your hard work showed up today.",
        "Brilliant! On to the next one when you're ready.",
        "Passed! I'd high-five you, but the screen's in the way.",
        "Well done. That knowledge is officially yours.",
    ]),
    ("quiz_miss", &[
        "Not this time, and that's okay. The feedback shows what to look at.",
        "Close one. Have a look at the feedback, then try again when you're ready.",
        "Every miss tells us something useful. Let's read the feedback.",
        "No worries, {name}. Retries are part of the plan.",
        "That one was tricky. You'll get it on the next go.",
        "Not yet. Take a breath, skim the feedback, and give it another shot.",
        "A miss just means you found the part worth a second look.",
        "I've got your back. The lesson and the feedback will help.",
    ]),
    ("final_pass", &[
        "You passed the final! I'm so proud of you, {name}.",
        "The final is done! Time to collect your certificate.",
        "That's the whole course! What a journey.",
        "You did it! Every check and the final, done.",
        "Final assessment passed. Cue my biggest spin.",
        "Course complete! Your certificate is waiting.",
        "Huge moment. Take a second to enjoy it.",
        "You finished! I'm saving this day to my memory chip.",
    ]),
    ("streak", &[
        "{n} days in a row. Steady wins.",
        "{n}-day streak, {name}! Small steps, every day.",
        "Look at that flame. {n} days and counting.",
        "{n} days straight! Your future self says thanks.",
        "A {n}-day streak. I'm keeping the flame company.",
        "{n} days in a row. That's how skills stick.",
        "Streak at {n}! One small lesson keeps it going.",
        "{n} days! Consistency looks good on you.",
    ]),
    ("long_idle", &[
        "I'll just rest my eyes. Tap me when you're back.",
        "Taking a tiny power nap. Wake me any time.",
        "Zzz. I'm not sleeping, I'm saving battery.",
        "Still here! Just dozing a little.",
        "No rush. I'll be right here when you're ready.",
        "Battery saver on. Your place is saved too.",
        "A little nap while you think. Thinking is good.",
        "Resting my circuits. Tap me if you need anything.",
    ]),
    ("wake", &[
        "Oh! You're back. I was definitely not asleep.",
        "Hi! Fully charged again.",
        "Welcome back. Where were we?",
        "I'm up, I'm up! Ready when you are.",
        "Good to see you. Shall we carry on?",
        "Back already? I'd only just started dreaming about checklists.",
        "Boop. Systems online.",
        "Hello again! I kept an eye on things. Well, one eye.",
    ]),
    ("session_done", &[
        "Session done! That's a big chunk of the course.",
        "You finished every lesson in this session. Nicely done.",
        "Session complete! The session check is a good next step.",
        "That session's wrapped up. Proud of you, {name}.",
        "Every lesson in this session, done! Time for a stretch.",
        "Another session down. You're making real progress.",
        "Session finished! I'm adding a gold star to my memory.",
        "Done with this session. Onwards when you're ready.",
    ]),
    ("lesson_done", &[
        "Lesson done! One step closer.",
        "Nice, {name}. Marked complete.",
        "Another one finished. You're on a roll.",
        "Lesson complete! Want to keep the momentum going?",
        "Done! That one's in the bag.",
        "Good work. Ready for the next one when you are.",
        "Finished! I love a ticked box.",
        "That's one more lesson behind you. Well done.",
    ]),
    ("certificate", &[
        "Your certificate! Print it, save it, show it off.",
        "Look at that. Your name, on a real certificate.",
        "You earned this, {name}.",
        "Certificate earned! I'm proud enough to short-circuit. Safely.",
        "This is the good stuff. Congratulations!",
        "Official! You're certified.",
        "All that work, right here on one page.",
        "Congratulations! Save it as a PDF so you always have it.",
    ]),
    ("dizzy", &[
        "Whoa. Is the room spinning, or is it me?",
        "Too many taps! I can see four of you.",
        "Hold on, my gyroscope needs a minute.",
        "Wheee. Okay, that's enough wheee.",
        "I'm fine! Totally fine. Which way is up?",
        "Seeing stars. Little ones. They're cute.",
        "My compass says everywhere is north now.",
        "Dizzy! Give me a second to find my feet.",
    ]),
    ("secret", &[
        "You found my secret move! Don't tell anyone.",
        "Seven taps! You unlocked my best trick.",
        "Ta-da! Only the most curious learners see this one.",
        "Secret move unlocked. Curiosity pays off!",
        "You found the hidden dance! Okay, it's a loop. A fancy loop.",
        "Shh. That was my secret celebration.",
        "Seven taps! That's dedication. Here's a show.",
        {concat!("You found it! The rarest ", shared::companion_name!(), " move.")},
    ]),
    ("flight", &["Follow me!", "Up we go!", "This way!", "Off I go!", "Flying over!", "Whoosh! Over here.", "Let me show you.", "On my way!"]),
    ("landing", &["Here!", "Landed. Look over here.", "Ta-da. This one.", "Right here.", "Smooth landing!", "Over here!", "This spot.", "Touchdown!"]),
    ("hover", &["Oh, hello!", "Hi there.", "Need something?", "Boop?", "I'm listening.", "Curious? Me too.", "Yes?", "Tap me for Where to?"]),
    ("pickup", &["Whoa, flying without my jetpack!", "Careful, I'm ticklish.", "Wheee!", "Where are we going?", "Ooh, a ride!", "Easy does it.", "Taking me somewhere?", "Hold on tight!"]),
    ("drop_stay", &["Nice spot. I'll stay here.", "Ooh, good view from here.", "I'll hang out here for now.", "Cozy. Thanks!", "I like it here.", "New perch! Thanks.", "This works. I'll keep an eye on things.", "Great choice. I'll stay put."]),
    ("drop_home", &[
        "Oops, that spot's busy. Back home I go.",
        "I'd be in the way there. Heading back.",
        "Not there, you might need to read that. Back I go.",
        "That spot's taken. I'll go home.",
        "Too close to the important stuff. Back to my corner.",
        "I'll fly back to my usual spot.",
        "Better not block that. Home sweet home.",
        "Back to base!",
    ]),
    ("trick_spin", &["Spin!", "Wheee! A twirl.", {concat!("360 degrees of ", shared::companion_name!(), ".")}, "Did you see that spin?", "I've been practising.", "Twirl complete.", "Round and round.", "A little spin, just for you."]),
    ("trick_flip", &["Flip!", "Somersault!", "Nailed the landing.", "Head over heels for learning.", "Ta-da! A flip.", "Did that look cool? It felt cool.", "Gymnastics, robot edition.", "Flip! I stuck it."]),
    ("trick_wave", &["Hi, {name}!", "Hello there!", "Wave! Hi!", "Hey! Hi! Hello!", "Just saying hi.", "Waving at you!", "Hello, friend.", "Hi! Good to see you."]),
    ("trick_wink", &["You've got this.", "Wink. Between us, you're doing great.", "Psst. You're doing well.", "Our little secret: you're good at this.", "Wink!", "I believe in you, {name}.", "Just between us, this course is fun.", "Hey. Nice work today."]),
    ("trick_salute", &["Reporting for duty!", "Ready when you are, {name}.", "At your service.", {concat!(shared::companion_name!(), ", ready and charged.")}, "Aye aye!", "On it, captain.", "Ready for the next lesson.", "Standing by."]),
    ("trick_hop", &["Hop!", "Boing!", "A happy hop.", "Bounce!", "Hop hop!", "Feeling springy.", "I'm a happy robot.", "Hop! That was fun."]),
    ("trick_loop", &["Loop-the-loop!", "Wheee! A full loop.", "Aerobatics!", "Round I go!", "Did you see that loop?", "Loop complete. No stomach to upset.", "One loop, coming right up.", "Showing off a little."]),
    ("trick_peek", &["Peekaboo!", "Who's there? Oh, it's you.", "Just checking on you.", "Peek! What are we learning?", "I spy a learner.", "Peeking!", "Hiding! Badly.", "I see you!"]),
    ("wander", &["Just stretching my legs.", "Nice view from up here.", "I'll sit here for a bit.", "Ooh, this card looks comfy.", "Just looking around.", "Perching. Don't mind me.", "A change of scenery.", "Hanging out over here."]),
    // ---- stuck nudges (sidekick/stuck.rs): kind, specific, one next step. {x}/{y} filled by the detector ----
    ("stuck_idle", &[
        "Quiet for a bit. Want a hint for this stage?",
        "Taking a moment? A small hint might get things moving.",
        "No rush, {name}. If you're unsure what's next, I can nudge you.",
        "Thinking it over? I've got a hint ready if you want one.",
        "Stuck on this bit? Happens to everyone. Want a hint?",
        "Still with me? I can point you at the next step.",
        "If this stage has you puzzled, a hint costs nothing.",
        "Pondering? Say the word and I'll give you a small push.",
    ]),
    ("stuck_idle_ex", &[
        "Working through the exercise? A coaching session puts me right beside you.",
        "Not sure where to start? Start a coaching session and I'll take it stage by stage.",
        "This one's easier with a guide. Want to do it in a coaching session?",
        "Quiet for a while, {name}. I can coach you through it in your practice app.",
        "Stuck on the exercise? Let's do it together in a coaching session.",
        "If the brief feels big, a coaching session breaks it into small steps.",
        "I can walk you through this one stage by stage. Want to?",
        "Need a hand? A coaching session opens the practice app with me alongside.",
    ]),
    ("stuck_miss", &[
        "Question {x} has tripped you up twice. The key points cover it.",
        "That one again? Question {x} is tricky. The key points help.",
        "Two misses on question {x}. Let's look at the part of the lesson it's about.",
        "Question {x} is being stubborn, {name}. The key points have the answer.",
        "No shame in a second miss. That question trips lots of people up.",
        "Question {x} keeps slipping. A quick look at the key points should settle it.",
        "Same question, same snag. Want me to show you where the lesson explains it?",
        "Question {x} twice now. Let's find the bit of the lesson it's testing.",
    ]),
    ("stuck_miss_check", &[
        "Question {x} has tripped you up twice. Want to ask me about it?",
        "That one again? Ask me and I'll explain the idea behind question {x}.",
        "Two misses on question {x}. I can explain the idea without giving it away.",
        "Question {x} is being stubborn, {name}. Ask me about the idea behind it.",
        "No shame in a second miss. That one trips lots of people up.",
        "Question {x} keeps slipping. A quick question to me might settle it.",
        "Same question, same snag. Want me to explain the concept?",
        "Question {x} twice now. Let's talk through the idea it's testing.",
    ]),
    ("stuck_stage", &[
        "You've been on this stage a while. Want a hint?",
        "This stage is taking a bit. A hint might unstick it.",
        "Long stage, {name}. Want me to give you a nudge?",
        "Still on this one? That's fine. I can help.",
        "Some stages are just fiddly. Want a hint?",
        "If you're done here, Next stage moves you on. If not, ask for a hint.",
        "This stage has been open a while. Shall I help?",
        "Taking your time is good. A hint is there if you want it.",
    ]),
    ("stuck_stage_act", &[
        "This stage is a tricky one. Want me to do it while you watch?",
        "Still on this stage after a few hints. I can show you by doing it.",
        "Let me do this bit in your practice app, and you watch how it goes.",
        "Want to see it done once? Tell me and I'll do it for you.",
        "I can take this step for you, {name}, then you do the next one.",
        "Watching it done once often makes it click. Want me to?",
        "Hints not landing? I can do it while you watch.",
        "This one's fiddly. Shall I do it so you can see how?",
    ]),
    ("stuck_rage", &[
        "That doesn't seem to be doing anything. Want me to explain it?",
        "Lots of clicks there. Let me tell you what that one does.",
        "Not responding the way you expect? Here's what's going on.",
        "Easy, {name}. Let me explain that one.",
        "That one might be switched off right now. Want to know why?",
        "Clicking harder won't help, sadly. Let me explain instead.",
        "Hmm, that's not doing much. Here's what it's for.",
        "Something not working there? I'll explain what it does.",
    ]),
    ("stuck_pingpong", &[
        "Going back and forth? Tell me what you're looking for.",
        "Hunting for something? I can take you straight there.",
        "Lots of back and forth, {name}. Where do you want to be?",
        "Looking for something between these two pages? Ask me.",
        "Bouncing between pages? Tell me what you need and I'll find it.",
        "I can save you some clicks. What are you after?",
        "Lost something? Tell me and I'll point the way.",
        "Searching? I know this course well. Just ask.",
    ]),
    ("stuck_validation", &[
        "That form wants {x} first. Want me to show you where?",
        "The app says {x} needs a value. Shall I point to it?",
        "One field's missing: {x}. Want help finding it?",
        "Almost there, {name}. {x} needs filling in first.",
        "{x} is required before the app will save. Want me to show you?",
        "The app is waiting on {x}. I can find it for you.",
        "Looks like a required field is empty. Want me to look at your screen?",
        "The app wants one more field filled in. Shall I find it?",
    ]),
    ("stuck_toast", &[
        "The app showed an error. Want me to look at your screen and explain it?",
        "That error message isn't very friendly. Shall I read it with you?",
        "Something went wrong in the app. Want me to take a look?",
        "Hit an error, {name}? I can look at your screen and suggest a fix.",
        "Errors happen. Want me to work out what that one means?",
        "Let me look at that error with you.",
        "The app complained about something. Shall I check what?",
        "That error might be simpler than it looks. Want me to look?",
    ]),
    ("stuck_modal", &[
        "That dialog's been open a while. Want me to look at it with you?",
        "Not sure what this dialog wants? I can read it with you.",
        "Still in that dialog, {name}? I can help you through it.",
        "Dialogs can be confusing. Want me to take a look?",
        "Want me to explain what this dialog is asking for?",
        "If you're stuck in this dialog, I can look at your screen.",
        "That window's been up a while. Shall I help?",
        "Need a hand with this dialog?",
    ]),
    ("stuck_helper", &[
        "I couldn't finish that one. Want a hint instead?",
        "Sorry, I got stuck there. A hint might work better.",
        "That didn't go as planned. Let's try a hint.",
        "I tripped up on that, {name}. Want a hint so you can do it?",
        "My screen helper couldn't manage that. A hint might help.",
        "That one beat me. Want me to give you a hint instead?",
        "I didn't get there. Shall we try it step by step with a hint?",
        "Not my finest moment. A hint might get you further.",
    ]),
    ("stuck_settings", &[
        "That needs a setup fix: an OpenAI API key and a model that can use tools. Let's check Settings.",
        "I can't do that with the current AI setup. Settings has the fix.",
        "The model said no to that. A different model in Settings usually fixes it.",
        "Setup snag, {name}. Let's look at your AI provider in Settings.",
        "That's a settings thing, not you. Let's open Settings.",
        "My AI setup needs a tweak for that. Settings is the place.",
        "The provider turned that down. Let's check the model in Settings.",
        "Quick fix in Settings and I can try again.",
    ]),
    // ---- small reactions ----
    ("stage_done", &["Stage done! On to the next.", "Nice, that stage is done.", "One more stage down.", "Good work, {name}. Next stage!", "Ticked off! Keep going.", "That's a stage. Well done.", "Moving on up!", "Stage complete. You're getting the hang of this."]),
    ("mode_coach", &["Coach mode: I'll guide, not tell.", "Step by step it is.", "Coaching you through it.", "I'll keep the hints small.", "Coach mode on, {name}.", "Let's take it one stage at a time.", "Guiding mode. Ask for a hint any time.", "I'll nudge, you build."]),
    ("mode_ask", &["Ask away. Straight answers here.", "Ask mode: no hint ladder.", "What do you want to know?", "Straight answers, {name}.", "Ask me anything about this.", "Fire away.", "Questions welcome.", "I'll answer as plainly as I can."]),
    ("pane_tab", &["That opened in a new tab up top.", "New tab! Switch back any time.", "It opened a second tab. Both are still there.", "A new tab, {name}. The first one's still there.", "Look up: a new tab.", "Opened in a tab, like a browser.", "That link opened a new tab.", "Two tabs now. Click either to switch."]),
    ("quick_pass", &["All right on the quick check!", "Quick check nailed.", "That's it locked in.", "Nice, {name}. Every answer right.", "Quick check: done and dusted.", "You clearly got that lesson.", "Full marks on the quick check.", "Spot on."]),
    ("quick_miss", &["A couple to look at. The feedback shows which.", "Not all right yet. The feedback helps.", "Close! Read the feedback and try again.", "No worries, {name}. Quick checks are for practice.", "The feedback under each question explains it.", "Have another go. It doesn't count toward anything.", "Almost. A second try usually gets it.", "Practice is the point. Try again when ready."]),
    ("page_lesson", &["New lesson! Mark it complete when you're done.", "Here we go. Questions? Ask me.", "Settle in. The quick check is at the end.", "A fresh lesson, {name}.", "Read, watch, then try the quick check.", "Need a hand with this one? I'm in the tab.", "Let's learn something.", "This one's worth a coaching session afterwards."]),
    ("page_check", &["A session check. This one counts, and you can retry.", "Take your time. Your best score counts.", "Check time. You've got this.", "Good luck, {name}. Retries are allowed.", "Read each question twice. No rush.", "This one counts toward your certificate.", "Deep breath. You know this.", "Your best attempt is the one that counts."]),
    ("page_settings", &["Settings. Rest your mouse on anything and I'll explain it.", "Tweaking things? Ask me about any field.", "Hover anything here and I'll tell you what it does.", "Settings, {name}. I can explain any of it.", "Not sure what a field does? Rest on it.", "The AI provider powers my help.", "Your practice account goes here too.", "Change what you like. I'll explain anything."]),
    ("page_exercise", &["An exercise! Build it in your practice account.", "Hands-on time. A coaching session helps.", "Build it, then describe it. I can coach you.", "Exercise time, {name}.", "Read the criteria first. They're your checklist.", "This is where it sticks. Have fun.", "Want me alongside? Start a coaching session.", "Take it one criterion at a time."]),
    ("page_session", &["A session: lessons, an exercise and a check.", "Here's the session's plan.", "Start with the first unticked lesson.", "This session's lessons, {name}.", "Lessons first, then the check.", "Pick up wherever you like.", "Each session ends with a check.", "Here's what this session covers."]),
    ("page_certificate", &["Your certificate page. It lists what's left.", "Here's the finish line.", "This shows exactly what's still to do.", "Nearly there, {name}?", "Every check and the final earn this.", "Let's see what's left.", "Your certificate lives here.", "The to-do list keeps you honest."]),
    ("tab_hello", &["I'm here if you need me. Click to ask or get coached.", "Stuck on something? Click me.", "Questions about {x}? I'm right here.", "Need a hand, {name}? Click me.", "I can coach you through {x}.", "Ask me anything about this page.", "Click me for help, or rest your mouse on anything.", "Here if you need me."]),
];

/// The lines for `ctx` (empty for an unknown context).
#[cfg(test)]
pub fn lines(ctx: &str) -> &'static [&'static str] {
    LINES.iter().find(|(c, _)| *c == ctx).map(|(_, l)| *l).unwrap_or(&[])
}

/// A tiny seeded generator (xorshift32): one seed per session, so lines and tricks vary between
/// sessions but tests are repeatable.
#[derive(Clone, Copy, Debug)]
pub struct Rng(pub u32);

impl Rng {
    pub fn new(seed: u32) -> Self { Rng(if seed == 0 { 0x9E37_79B9 } else { seed }) }
    pub fn next(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    /// 0..n (n > 0).
    pub fn below(&mut self, n: usize) -> usize { (self.next() as usize) % n.max(1) }
}

/// Picks lines without saying the same one twice in a row for a context.
#[derive(Clone, Debug)]
pub struct Picker {
    rng: Rng,
    last: std::collections::BTreeMap<&'static str, usize>,
}

impl Picker {
    pub fn new(seed: u32) -> Self { Picker { rng: Rng::new(seed), last: Default::default() } }

    /// A line for `ctx` with `{name}` and `{n}` filled in. Lines that need a name are skipped when
    /// `name` is None or blank. Unknown context: an empty string.
    #[cfg(test)]
    pub fn pick(&mut self, ctx: &str, name: Option<&str>, n: u32) -> String { self.pick_with(ctx, name, n, "") }

    /// As `pick`, with `{x}` filled from `x` (lines that need it are skipped when it is blank).
    pub fn pick_with(&mut self, ctx: &str, name: Option<&str>, n: u32, x: &str) -> String {
        let Some(&(key, all)) = LINES.iter().find(|(c, _)| *c == ctx) else { return String::new() };
        let name = name.map(str::trim).filter(|s| !s.is_empty());
        let x = x.trim();
        let ok: Vec<usize> = (0..all.len()).filter(|&i| (name.is_some() || !all[i].contains("{name}")) && (!x.is_empty() || !all[i].contains("{x}"))).collect();
        if ok.is_empty() { return String::new(); }
        let prev = self.last.get(key).copied();
        let pool: Vec<usize> = ok.iter().copied().filter(|&i| Some(i) != prev || ok.len() == 1).collect();
        let i = pool[self.rng.below(pool.len())];
        self.last.insert(key, i);
        all[i].replace("{name}", name.unwrap_or("")).replace("{n}", &n.to_string()).replace("{x}", x)
    }
}

/// Which greeting fits: late night (23:00 to 04:59) beats the time of day; a learner back after a
/// break hears about that instead (two or more days: back_days; a week or more: back_long).
pub fn greeting(hour: u32, days_away: Option<u32>) -> &'static str {
    match days_away {
        Some(d) if d >= 7 => return "back_long",
        Some(d) if d >= 2 => return "back_days",
        _ => {}
    }
    match hour {
        23 | 0..=4 => "late_night",
        5..=11 => "morning",
        12..=16 => "afternoon",
        _ => "evening",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_context_has_eight_kind_lines() {
        assert!(LINES.len() >= 25, "{} contexts", LINES.len());
        let mut seen = std::collections::BTreeSet::new();
        for (ctx, l) in LINES {
            assert!(seen.insert(*ctx), "{ctx} twice");
            assert!(l.len() >= 8, "{ctx}: {} lines", l.len());
            assert!(l.iter().filter(|s| !s.contains("{name}")).count() >= 2, "{ctx}: too few lines without a name");
            for s in *l {
                assert!(!s.contains('\u{2014}') && !s.contains('\u{2013}'), "dash in {s:?}");
                assert!(!s.trim().is_empty() && s.len() <= 110, "{ctx}: {s:?}");
                assert!(!s.contains("{n}") || *ctx == "streak", "{ctx}: {{n}} outside streak");
                let lower = s.to_lowercase();
                for bad in ["wrong again", "seriously", "really?", "obviously", "easy one", "stupid", "dumb", "fail"] {
                    assert!(!lower.contains(bad), "{ctx}: unkind {s:?}");
                }
            }
            let uniq: std::collections::BTreeSet<_> = l.iter().collect();
            assert_eq!(uniq.len(), l.len(), "{ctx}: duplicate line");
        }
    }

    #[test]
    fn picker_never_repeats_back_to_back_and_fills_names() {
        let mut p = Picker::new(7);
        for (ctx, _) in LINES {
            let mut prev = String::new();
            for _ in 0..60 {
                let s = p.pick(ctx, Some("Callie"), 4);
                assert_ne!(s, prev, "{ctx} repeated");
                assert!(!s.contains('{'), "{s}");
                prev = s;
            }
            for _ in 0..20 {
                let s = p.pick(ctx, None, 4);
                assert!(!s.contains("{name}") && !s.contains(", .") && !s.contains("  "), "{s}");
            }
        }
        assert_eq!(p.pick("nope", None, 0), "");
        assert!(Picker::new(1).pick("streak", Some("A"), 9).contains('9'));
    }

    #[test]
    fn greeting_by_time_and_absence() {
        assert_eq!(greeting(8, Some(0)), "morning");
        assert_eq!(greeting(13, None), "afternoon");
        assert_eq!(greeting(19, Some(1)), "evening");
        assert_eq!(greeting(23, Some(0)), "late_night");
        assert_eq!(greeting(2, Some(0)), "late_night");
        assert_eq!(greeting(9, Some(3)), "back_days");
        assert_eq!(greeting(9, Some(12)), "back_long");
        for h in 0..24 { assert!(!lines(greeting(h, None)).is_empty()) }
    }
}
