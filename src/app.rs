use std::{collections::HashSet, path::PathBuf};

use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, IconName, Sizable as _, TitleBar, WindowExt as _,
    button::{Button, ButtonVariant, ButtonVariants as _},
    checkbox::Checkbox,
    h_flex,
    input::{Input, InputState},
    notification::Notification,
    scroll::ScrollableElement as _,
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use uuid::Uuid;

use crate::{
    TokioRuntime,
    recorder::Recording,
    settings::Settings,
    store::{Store, Transcript},
    transcriber,
};

enum State {
    Idle,
    Recording(Recording),
    Transcribing,
}

pub struct TranscriptsView {
    store: Store,
    selected: HashSet<Uuid>,
    state: State,
    /// Audio from the last failed transcription, kept so it can be retried.
    failed_audio: Option<Vec<u8>>,
    settings: Settings,
    settings_path: PathBuf,
}

impl TranscriptsView {
    pub fn new(store: Store, settings: Settings, settings_path: PathBuf) -> Self {
        Self {
            store,
            selected: HashSet::new(),
            state: State::Idle,
            failed_audio: None,
            settings,
            settings_path,
        }
    }

    fn toggle_recording(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match std::mem::replace(&mut self.state, State::Idle) {
            State::Idle if self.settings.api_key().is_none() => {
                window.push_notification("Add your OpenAI API key in Settings", cx);
                self.open_settings(window, cx);
            }
            State::Idle => match Recording::start() {
                Ok(rec) => {
                    self.failed_audio = None;
                    self.state = State::Recording(rec);
                }
                Err(e) => notify_error(window, cx, format!("Could not start recording: {e:#}")),
            },
            State::Recording(rec) => match rec.stop() {
                Ok(wav) => self.start_transcription(wav, window, cx),
                Err(e) => notify_error(window, cx, format!("Recording failed: {e:#}")),
            },
            State::Transcribing => self.state = State::Transcribing,
        }
        cx.notify();
    }

    fn start_transcription(&mut self, wav: Vec<u8>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(api_key) = self.settings.api_key() else {
            notify_error(window, cx, "No OpenAI API key configured".into());
            self.failed_audio = Some(wav);
            return;
        };
        self.failed_audio = None;
        self.state = State::Transcribing;
        let job = cx
            .global::<TokioRuntime>()
            .0
            .spawn(transcriber::transcribe(api_key, wav.clone()));

        cx.spawn_in(window, async move |this, cx| {
            let result = match job.await {
                Ok(r) => r,
                Err(e) => Err(e.into()),
            };
            this.update_in(cx, |this, window, cx| {
                this.state = State::Idle;
                match result {
                    Ok(text) if text.is_empty() => {
                        window.push_notification("No speech detected", cx)
                    }
                    Ok(text) => {
                        this.store.add(Transcript::new(text));
                        this.persist(window, cx);
                    }
                    Err(e) => {
                        this.failed_audio = Some(wav);
                        notify_error(window, cx, format!("Transcription failed: {e:#}"));
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn retry_transcription(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(wav) = self.failed_audio.take() {
            self.start_transcription(wav, window, cx);
        }
        cx.notify();
    }

    fn persist(&self, window: &mut Window, cx: &mut App) {
        if let Err(e) = self.store.save() {
            notify_error(window, cx, format!("Could not save transcripts: {e:#}"));
        }
    }

    fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let current = self.settings.openai_api_key.clone().unwrap_or_default();
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .masked(true)
                .placeholder("sk-...")
                .default_value(current)
        });
        let hint: SharedString = format!("Saved to {}", self.settings_path.display()).into();
        let view = cx.entity().downgrade();

        window.open_dialog(cx, move |dialog, _, cx| {
            let input = input.clone();
            let view = view.clone();
            dialog
                .title("Settings")
                .width(px(460.))
                .child(
                    v_flex()
                        .gap_2()
                        .child(div().text_sm().child("OpenAI API key"))
                        .child(Input::new(&input).mask_toggle())
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(hint.clone()),
                        ),
                )
                .footer(
                    h_flex()
                        .w_full()
                        .gap_2()
                        .justify_end()
                        .child(
                            Button::new("settings-cancel")
                                .outline()
                                .label("Cancel")
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("settings-save")
                                .primary()
                                .label("Save")
                                .on_click(move |_, window, cx| {
                                    let key = input.read(cx).value().trim().to_string();
                                    let saved = view.update(cx, |this, cx| {
                                        this.settings.openai_api_key =
                                            (!key.is_empty()).then_some(key);
                                        cx.notify();
                                        this.settings.save(&this.settings_path)
                                    });
                                    match saved {
                                        Ok(Ok(())) => {
                                            window.close_dialog(cx);
                                            window.push_notification(
                                                Notification::success("Settings saved"),
                                                cx,
                                            );
                                        }
                                        Ok(Err(e)) => notify_error(
                                            window,
                                            cx,
                                            format!("Could not save settings: {e:#}"),
                                        ),
                                        Err(_) => window.close_dialog(cx),
                                    }
                                }),
                        ),
                )
        });
    }

    fn copy_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self
            .store
            .items
            .iter()
            .filter(|t| self.selected.contains(&t.id))
            .map(|t| t.text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");
        copy(text, window, cx);
    }

    fn confirm_delete(&mut self, id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let view = view.clone();
            alert
                .title("Delete transcript?")
                .description("This transcript will be permanently removed.")
                .show_cancel(true)
                .ok_text("Delete")
                .ok_variant(ButtonVariant::Danger)
                .on_ok(move |_, window, cx| {
                    view.update(cx, |this, cx| {
                        this.store.remove(id);
                        this.selected.remove(&id);
                        this.persist(window, cx);
                        cx.notify();
                    })
                    .ok();
                    true
                })
        });
    }

    fn render_card(&self, t: &Transcript, cx: &mut Context<Self>) -> impl IntoElement {
        let id = t.id;
        let theme = cx.theme();
        let text = t.text.clone();

        v_flex()
            .gap_2()
            .p_3()
            .rounded_lg()
            .border_1()
            .border_color(theme.border)
            .bg(theme.background)
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        Checkbox::new(SharedString::from(format!("select-{id}")))
                            .checked(self.selected.contains(&id))
                            .on_click(cx.listener(move |this, checked: &bool, _, cx| {
                                if *checked {
                                    this.selected.insert(id);
                                } else {
                                    this.selected.remove(&id);
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(t.created_at.format("%Y-%m-%d %H:%M:%S").to_string()),
                    )
                    .child(
                        Button::new(SharedString::from(format!("copy-{id}")))
                            .ghost()
                            .small()
                            .icon(IconName::Copy)
                            .tooltip("Copy")
                            .on_click(move |_, window, cx| copy(text.clone(), window, cx)),
                    )
                    .child(
                        Button::new(SharedString::from(format!("delete-{id}")))
                            .ghost()
                            .xsmall()
                            .icon(IconName::Close)
                            .tooltip("Delete")
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.confirm_delete(id, window, cx)
                            })),
                    ),
            )
            .child(div().text_base().child(t.text.clone()))
    }
}

impl Render for TranscriptsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (label, transcribing) = match self.state {
            State::Idle => ("Listen", false),
            State::Recording(_) => ("Stop", false),
            State::Transcribing => ("Transcribing…", true),
        };
        let recording = matches!(self.state, State::Recording(_));
        let show_retry = matches!(self.state, State::Idle) && self.failed_audio.is_some();
        let n_selected = self.selected.len();

        let mut record = Button::new("record")
            .label(label)
            .loading(transcribing)
            .disabled(transcribing)
            .on_click(cx.listener(|this, _, window, cx| this.toggle_recording(window, cx)));
        record = if recording {
            record.danger()
        } else {
            record.primary()
        };

        let header = h_flex()
            .gap_2()
            .p_4()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(record)
            .when(show_retry, |header| {
                header.child(
                    Button::new("retry")
                        .outline()
                        .icon(IconName::RefreshCw)
                        .tooltip("Retry transcription")
                        .on_click(
                            cx.listener(|this, _, window, cx| this.retry_transcription(window, cx)),
                        ),
                )
            })
            .child(div().flex_1())
            .child(
                Button::new("copy-selected")
                    .outline()
                    .icon(IconName::Copy)
                    .label(format!("Copy selected ({n_selected})"))
                    .disabled(n_selected == 0)
                    .on_click(cx.listener(|this, _, window, cx| this.copy_selected(window, cx))),
            )
            .child(
                Button::new("settings")
                    .ghost()
                    .icon(IconName::Settings)
                    .tooltip("Settings")
                    .on_click(cx.listener(|this, _, window, cx| this.open_settings(window, cx))),
            );

        let body = if self.store.items.is_empty() {
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .items_center()
                .justify_center()
                .text_color(cx.theme().muted_foreground)
                .child("No transcripts yet — press Listen")
                .into_any_element()
        } else {
            let cards: Vec<_> = self
                .store
                .items
                .iter()
                .map(|t| self.render_card(t, cx).into_any_element())
                .collect();
            // Bounded outer box so the list overflows instead of growing the window.
            div()
                .flex_1()
                .min_h_0()
                .overflow_hidden()
                .child(
                    v_flex()
                        .id("transcripts")
                        .size_full()
                        .overflow_y_scrollbar()
                        .gap_3()
                        .p_4()
                        .children(cards),
                )
                .into_any_element()
        };

        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(TitleBar::new().child(div().text_sm().child("Speech to Text")))
            .child(header)
            .child(body)
    }
}

fn copy(text: String, window: &mut Window, cx: &mut App) {
    cx.write_to_clipboard(ClipboardItem::new_string(text));
    window.push_notification(Notification::success("Copied to clipboard"), cx);
}

fn notify_error(window: &mut Window, cx: &mut App, message: String) {
    window.push_notification(Notification::error(message), cx);
}
