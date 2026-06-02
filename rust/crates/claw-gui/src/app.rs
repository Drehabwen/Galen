use crate::backend::{ChatBackend, StreamEvent};
use eframe::egui;
use tokio::sync::mpsc;

#[derive(PartialEq)]
enum AppMode {
    Placeholder,
    Chatting,
}

pub struct ClawMdApp {
    backend: ChatBackend,
    messages: Vec<ChatMessage>,
    input: String,
    mode: AppMode,
    stream_rx: Option<mpsc::UnboundedReceiver<StreamEvent>>,
    streaming_text: String,
    current_model: String,
    available_models: Vec<String>,
    error_text: Option<String>,
    // Left panel state
    left_content: String,
}

#[derive(Clone)]
struct ChatMessage {
    role: String,
    content: String,
}

fn load_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    let zh_font_paths = [
        "C:\\Windows\\Fonts\\msyh.ttc",
        "C:\\Windows\\Fonts\\simhei.ttf",
    ];

    let mut loaded = false;
    for path in &zh_font_paths {
        if let Ok(bytes) = std::fs::read(path) {
            let font_data =
                std::sync::Arc::new(egui::FontData::from_owned(bytes).tweak(
                    egui::FontTweak {
                        scale: 1.0,
                        ..Default::default()
                    },
                ));
            fonts.font_data.insert("zh_font".to_owned(), font_data);
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "zh_font".to_owned());
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .insert(0, "zh_font".to_owned());
            loaded = true;
            break;
        }
    }

    if !loaded {
        eprintln!("Warning: No Chinese font found. CJK characters may not render correctly.");
    }

    ctx.set_fonts(fonts);
}

impl ClawMdApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        load_fonts(&_cc.egui_ctx);

        let backend = ChatBackend::new();
        let default_model = backend
            .default_model()
            .map(|m| m.name)
            .unwrap_or_else(|| "claude-sonnet-4-6".to_string());

        let available_models = backend
            .all_models()
            .iter()
            .map(|m| m.name.clone())
            .collect();

        _cc.egui_ctx.set_visuals(egui::Visuals::dark());

        Self {
            backend,
            messages: vec![ChatMessage {
                role: "assistant".to_string(),
                content: "欢迎使用 VIBE Paper！我是你的科研助手。\n\n你可以直接问我问题，我会帮你检索文献、解释术语、格式化引用。\n\n试试问我：\n• 帮我查一下阿尔茨海默病的最新研究\n• 解释一下什么是单核苷酸多态性\n• 用 Vancouver 格式引用这篇 PMID: 12345678".to_string(),
            }],
            input: String::new(),
            mode: AppMode::Placeholder,
            stream_rx: None,
            streaming_text: String::new(),
            current_model: default_model,
            available_models,
            error_text: None,
            left_content: String::new(),
        }
    }

    fn send_message(&mut self) {
        let text = self.input.trim().to_string();
        if text.is_empty() {
            return;
        }

        self.messages.push(ChatMessage {
            role: "user".to_string(),
            content: text.clone(),
        });

        self.input.clear();
        self.mode = AppMode::Chatting;
        self.streaming_text.clear();
        self.error_text = None;

        let (tx, rx) = mpsc::unbounded_channel();
        self.stream_rx = Some(rx);

        let model_id = self.backend.resolve_model(&self.current_model);
        let history: Vec<_> = self
            .messages
            .iter()
            .filter(|m| m.role == "user" || m.role == "assistant")
            .map(|m| api::InputMessage {
                role: m.role.clone(),
                content: vec![api::InputContentBlock::Text {
                    text: m.content.clone(),
                }],
            })
            .collect();

        ChatBackend::spawn_chat(model_id, text, history, tx);
    }

    fn poll_stream(&mut self) {
        if let Some(ref mut rx) = self.stream_rx {
            loop {
                match rx.try_recv() {
                    Ok(StreamEvent::Delta(text)) => {
                        self.streaming_text.push_str(&text);
                    }
                    Ok(StreamEvent::Done(text)) => {
                        self.messages.push(ChatMessage {
                            role: "assistant".to_string(),
                            content: text,
                        });
                        self.streaming_text.clear();
                        self.stream_rx = None;
                        self.mode = AppMode::Placeholder;
                        break;
                    }
                    Ok(StreamEvent::Error(e)) => {
                        self.error_text = Some(e);
                        self.stream_rx = None;
                        self.mode = AppMode::Placeholder;
                        break;
                    }
                    Err(mpsc::error::TryRecvError::Empty) => break,
                    Err(mpsc::error::TryRecvError::Disconnected) => {
                        if !self.streaming_text.is_empty() {
                            self.messages.push(ChatMessage {
                                role: "assistant".to_string(),
                                content: std::mem::take(&mut self.streaming_text),
                            });
                        }
                        self.stream_rx = None;
                        self.mode = AppMode::Placeholder;
                        break;
                    }
                }
            }
        }
    }
}

impl eframe::App for ClawMdApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_stream();

        // Ctrl+Enter to send
        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) && ctx.input(|i| i.modifiers.ctrl) {
            self.send_message();
        }

        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("🦞 VIBE Paper");
                ui.separator();
                ui.label("模型:");
                egui::ComboBox::from_id_salt("model_select")
                    .selected_text(&self.current_model)
                    .show_ui(ui, |ui| {
                        for model in &self.available_models.clone() {
                            ui.selectable_value(
                                &mut self.current_model,
                                model.clone(),
                                model,
                            );
                        }
                    });
                ui.separator();
                if self.mode == AppMode::Chatting {
                    ui.spinner();
                    ui.label("思考中...");
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("新对话").clicked() {
                        self.messages.clear();
                        self.messages.push(ChatMessage {
                            role: "assistant".to_string(),
                            content: "新对话已开始。有什么可以帮你的？".to_string(),
                        });
                    }
                });
            });
        });

        egui::TopBottomPanel::bottom("bottom_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let response = ui.add_sized(
                    [ui.available_width() - 80.0, 32.0],
                    egui::TextEdit::singleline(&mut self.input)
                        .hint_text("输入消息，Ctrl+Enter 发送...")
                        .desired_width(f32::INFINITY),
                );
                if response.lost_focus() && ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                    self.send_message();
                }
                if ui.button("发送").clicked() || {
                    let enter_pressed = ctx.input(|i| i.key_pressed(egui::Key::Enter));
                    let has_focus = response.has_focus();
                    enter_pressed && has_focus && !ctx.input(|i| i.modifiers.ctrl)
                } {
                    self.send_message();
                    response.request_focus();
                }
                ui.add_space(8.0);
                let sending = self.mode == AppMode::Chatting;
                if ui.add_enabled(!sending && !self.input.trim().is_empty(), egui::Button::new("发送")).clicked() {
                    self.send_message();
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.columns(2, |columns| {
                // LEFT: Work area
                columns[0].vertical_centered(|ui| {
                    ui.add_space(20.0);
                    ui.heading("📄 工作区");
                    ui.add_space(10.0);
                    if self.left_content.is_empty() {
                        ui.label("在这里查看论文、编辑文档、预览模板");
                        ui.add_space(20.0);
                        ui.separator();
                        ui.add_space(10.0);
                        ui.label("📋 选中文字 → 右键解释");
                        ui.label("📚 粘贴 PMID → 加载摘要");
                        ui.label("📝 选择模板 → 开始写作");
                    } else {
                        ui.label(&self.left_content);
                    }
                });

                // RIGHT: Chat
                columns[1].vertical(|ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            for msg in &self.messages {
                                let (color, label) = match msg.role.as_str() {
                                    "user" => (egui::Color32::from_rgb(100, 149, 237), "🧑 你"),
                                    "assistant" => (egui::Color32::from_rgb(144, 238, 144), "🤖 VIBE Paper"),
                                    _ => (egui::Color32::GRAY, "❓"),
                                };

                                ui.horizontal(|ui| {
                                    ui.colored_label(color, label);
                                    ui.add_space(4.0);
                                });

                                let rich = egui::RichText::new(&msg.content).size(14.0);
                                ui.label(rich);
                                ui.add_space(8.0);
                                ui.separator();
                            }

                            if !self.streaming_text.is_empty() {
                                ui.horizontal(|ui| {
                                    ui.colored_label(
                                        egui::Color32::from_rgb(144, 238, 144),
                                        "🤖 Claw-MD",
                                    );
                                    ui.add_space(4.0);
                                    ui.spinner();
                                });
                                ui.label(&self.streaming_text);
                            }

                            if let Some(ref error) = self.error_text {
                                ui.colored_label(egui::Color32::RED, format!("❌ {error}"));
                            }
                        });
                });
            });
        });

        ctx.request_repaint();
    }
}
