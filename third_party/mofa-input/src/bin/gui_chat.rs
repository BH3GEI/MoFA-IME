use eframe::egui;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum ModelSize {
    Small,    // 0.5B
    Medium,   // 1.5B
    Large,    // 7B
    XLarge,   // 14B
}

impl ModelSize {
    fn path(&self) -> PathBuf {
        let base = dirs::home_dir()
            .map(|h| h.join(".mofa/models"))
            .unwrap_or_else(|| PathBuf::from("./models"));

        std::fs::create_dir_all(&base).ok();

        match self {
            ModelSize::Small => base.join("qwen2.5-0.5b-q4_k_m.gguf"),
            ModelSize::Medium => base.join("qwen2.5-1.5b-q4_k_m.gguf"),
            ModelSize::Large => base.join("qwen2.5-7b-q4_k_m.gguf"),
            ModelSize::XLarge => base.join("qwen2.5-14b-q4_k_m.gguf"),
        }
    }

    fn name(&self) -> &'static str {
        match self {
            ModelSize::Small => "0.5B",
            ModelSize::Medium => "1.5B",
            ModelSize::Large => "7B",
            ModelSize::XLarge => "14B",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            ModelSize::Small => "超快，适合简单任务 (~400MB)",
            ModelSize::Medium => "推荐，速度与质量均衡 (~1GB)",
            ModelSize::Large => "更智能，需更多内存 (~4.5GB)",
            ModelSize::XLarge => "最聪明，推理能力强 (~9GB)",
        }
    }

    fn size_mb(&self) -> u64 {
        match self {
            ModelSize::Small => 400,
            ModelSize::Medium => 1000,
            ModelSize::Large => 4500,
            ModelSize::XLarge => 9000,
        }
    }

    fn download_url(&self) -> &'static str {
        match self {
            ModelSize::Small => "https://huggingface.co/lmstudio-community/Qwen2.5-0.5B-Instruct-GGUF/resolve/main/Qwen2.5-0.5B-Instruct-Q4_K_M.gguf",
            ModelSize::Medium => "https://huggingface.co/lmstudio-community/Qwen2.5-1.5B-Instruct-GGUF/resolve/main/Qwen2.5-1.5B-Instruct-Q4_K_M.gguf",
            ModelSize::Large => "https://huggingface.co/lmstudio-community/Qwen2.5-7B-Instruct-GGUF/resolve/main/Qwen2.5-7B-Instruct-Q4_K_M.gguf",
            ModelSize::XLarge => "https://huggingface.co/lmstudio-community/Qwen2.5-14B-Instruct-GGUF/resolve/main/Qwen2.5-14B-Instruct-Q4_K_M.gguf",
        }
    }

    fn all() -> [ModelSize; 4] {
        [ModelSize::Small, ModelSize::Medium, ModelSize::Large, ModelSize::XLarge]
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum WhisperModelSize {
    Tiny,   // 72MB
    Base,   // 142MB
    Small,  // 466MB
    Medium, // 1.5GB
}

impl WhisperModelSize {
    fn path(&self) -> PathBuf {
        let base = dirs::home_dir()
            .map(|h| h.join(".mofa/models"))
            .unwrap_or_else(|| PathBuf::from("./models"));
        match self {
            WhisperModelSize::Tiny => base.join("ggml-tiny.bin"),
            WhisperModelSize::Base => base.join("ggml-base.bin"),
            WhisperModelSize::Small => base.join("ggml-small.bin"),
            WhisperModelSize::Medium => base.join("ggml-medium.bin"),
        }
    }

    fn name(&self) -> &'static str {
        match self {
            WhisperModelSize::Tiny => "Tiny",
            WhisperModelSize::Base => "Base",
            WhisperModelSize::Small => "Small",
            WhisperModelSize::Medium => "Medium",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            WhisperModelSize::Tiny => "超快 (~72MB)",
            WhisperModelSize::Base => "平衡 (~142MB)",
            WhisperModelSize::Small => "较好 (~466MB)",
            WhisperModelSize::Medium => "最佳 (~1.5GB)",
        }
    }

    fn size_mb(&self) -> u64 {
        match self {
            WhisperModelSize::Tiny => 72,
            WhisperModelSize::Base => 142,
            WhisperModelSize::Small => 466,
            WhisperModelSize::Medium => 1500,
        }
    }

    fn download_url(&self) -> &'static str {
        match self {
            WhisperModelSize::Tiny => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin",
            WhisperModelSize::Base => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin",
            WhisperModelSize::Small => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin",
            WhisperModelSize::Medium => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin",
        }
    }

    fn all() -> [WhisperModelSize; 4] {
        [WhisperModelSize::Tiny, WhisperModelSize::Base, WhisperModelSize::Small, WhisperModelSize::Medium]
    }
}

struct ChatMessage {
    role: String,
    content: String,
}

enum AppEvent {
    Token(String),
    GenerationComplete,
    ModelLoaded,
    Error(String),
    DownloadProgress(ModelSize, f32), // LLM model, percent
    DownloadComplete(ModelSize),
    DownloadError(ModelSize, String),
    // ASR events
    AsrDownloadProgress(WhisperModelSize, f32),
    AsrDownloadComplete(WhisperModelSize),
    AsrDownloadError(WhisperModelSize, String),
    AsrResult(WhisperModelSize, String),  // Model-specific result
    AsrModelError(WhisperModelSize, String), // Model-specific error
    AsrError(String),
}

struct ChatApp {
    chat: Option<mofa_input::llm::ChatSession>,
    messages: Vec<ChatMessage>,
    input: String,
    selected_model: ModelSize,
    loaded_model: Option<ModelSize>,
    is_loading: bool,
    is_generating: bool,
    status: String,
    token_count: i32,
    event_receiver: Receiver<AppEvent>,
    event_sender: Sender<AppEvent>,
    current_response: String,
    show_switch_confirm: bool,
    pending_model: Option<ModelSize>,
    download_progress: HashMap<ModelSize, f32>,
    downloading_models: HashSet<ModelSize>,
    show_download_manager: bool,
    show_delete_confirm: bool,
    pending_delete: Option<ModelSize>,

    // ASR fields - multiple models, each with its own text box
    asr_sessions: HashMap<WhisperModelSize, Arc<Mutex<mofa_input::asr::AsrSession>>>,
    asr_texts: HashMap<WhisperModelSize, String>, // Each model has its own text box
    asr_download_progress: HashMap<WhisperModelSize, f32>,
    asr_downloading_models: HashSet<WhisperModelSize>,
    is_recording: bool,
    audio_samples: Arc<Mutex<Vec<f32>>>,
    show_asr_manager: bool,
    asr_status: String,
    _recording_sample_rate: Arc<Mutex<u32>>, // Store actual recording sample rate for resampling
    recording_stop_signal: Option<std::sync::mpsc::Sender<()>>, // Signal to stop recording
    recording_thread_handle: Option<std::thread::JoinHandle<()>>, // Handle to wait for thread
}

impl ChatApp {
    fn new() -> Self {
        let (tx, rx) = channel();
        Self {
            chat: None,
            messages: Vec::new(),
            input: String::new(),
            selected_model: ModelSize::Medium,
            loaded_model: None,
            is_loading: false,
            is_generating: false,
            status: "请选择模型".to_string(),
            token_count: 0,
            event_receiver: rx,
            event_sender: tx,
            current_response: String::new(),
            show_switch_confirm: false,
            pending_model: None,
            download_progress: HashMap::new(),
            downloading_models: HashSet::new(),
            show_download_manager: false,
            show_delete_confirm: false,
            pending_delete: None,

            // ASR initialization
            asr_sessions: HashMap::new(),
            asr_texts: HashMap::new(),
            asr_download_progress: HashMap::new(),
            asr_downloading_models: HashSet::new(),
            is_recording: false,
            audio_samples: Arc::new(Mutex::new(Vec::new())),
            show_asr_manager: false,
            asr_status: "请选择语音模型".to_string(),
            _recording_sample_rate: Arc::new(Mutex::new(16000)),
            recording_stop_signal: None,
            recording_thread_handle: None,
        }
    }

    fn is_model_available(&self, model: ModelSize) -> bool {
        model.path().exists() && !self.downloading_models.contains(&model)
    }

    fn is_asr_model_available(&self, model: WhisperModelSize) -> bool {
        model.path().exists() && !self.asr_downloading_models.contains(&model)
    }

    fn has_download_tool() -> bool {
        use std::process::{Command, Stdio};
        Command::new("wget").arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok()
            || Command::new("curl").arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok()
    }

    fn cancel_download(&mut self, model: ModelSize) {
        self.downloading_models.remove(&model);
        self.download_progress.remove(&model);
        let path = model.path();
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
        self.status = format!("{} 下载已取消", model.name());
    }

    fn delete_model(&mut self, model: ModelSize) {
        if self.loaded_model == Some(model) {
            self.chat = None;
            self.loaded_model = None;
            self.token_count = 0;
        }
        let path = model.path();
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
        self.status = format!("{} 已删除", model.name());
    }

    fn download_model(&mut self, model: ModelSize) {
        if self.downloading_models.contains(&model) {
            return;
        }

        if !Self::has_download_tool() {
            self.status = "错误: 未找到wget或curl，请手动安装".to_string();
            return;
        }

        self.downloading_models.insert(model);
        let sender = self.event_sender.clone();
        let url = model.download_url().to_string();
        let path = model.path();

        std::thread::spawn(move || {
            // Create parent directory
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }

            // Download with progress
            match Self::download_with_progress(&url, &path, model, sender.clone()) {
                Ok(_) => {
                    let _ = sender.send(AppEvent::DownloadComplete(model));
                }
                Err(e) => {
                    let _ = sender.send(AppEvent::DownloadError(model, e));
                }
            }
        });
    }

    fn download_with_progress(
        url: &str,
        path: &PathBuf,
        model: ModelSize,
        sender: Sender<AppEvent>,
    ) -> Result<(), String> {
        use std::process::{Command, Stdio};
        use std::thread;
        use std::time::Duration;

        let path_str = path.to_string_lossy().to_string();
        let url = url.to_string();
        let expected_size = model.size_mb() * 1024 * 1024;

        let _ = sender.send(AppEvent::DownloadProgress(model, 0.0));

        // Try wget first, then curl
        let has_wget = Command::new("wget").arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok();
        let mut child = if has_wget {
            let mut c = Command::new("wget");
            c.args([&url, "-O", &path_str, "--timeout=60", "--tries=3", "-q"])
             .stdout(Stdio::null())
             .stderr(Stdio::null())
             .spawn()
             .map_err(|e| format!("启动wget失败: {}", e))?
        } else if Command::new("curl").arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok() {
            let mut c = Command::new("curl");
            c.args(["-L", "-o", &path_str, &url, "--connect-timeout", "60", "--max-time", "600", "-s"])
             .stdout(Stdio::null())
             .stderr(Stdio::null())
             .spawn()
             .map_err(|e| format!("启动curl失败: {}", e))?
        } else {
            return Err("未找到wget或curl，请手动安装".to_string());
        };

        let path_clone = path.clone();
        let sender_clone = sender.clone();
        let progress_handle = thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_millis(500));
                if let Ok(metadata) = std::fs::metadata(&path_clone) {
                    let downloaded = metadata.len();
                    let percent = (downloaded as f64 / expected_size as f64 * 100.0).min(99.0);
                    let _ = sender_clone.send(AppEvent::DownloadProgress(model, percent as f32));
                }
            }
        });

        let result = child.wait()
            .map_err(|e| format!("等待下载失败: {}", e))?;

        // Stop progress monitoring
        drop(progress_handle);

        if result.success() {
            let _ = sender.send(AppEvent::DownloadProgress(model, 100.0));
            Ok(())
        } else {
            Err("下载失败".to_string())
        }
    }

    // ===== ASR Functions =====

    fn download_asr_model(&mut self, model: WhisperModelSize) {
        if self.asr_downloading_models.contains(&model) {
            return;
        }

        if !Self::has_download_tool() {
            self.asr_status = "错误: 未找到wget或curl，请手动安装".to_string();
            return;
        }

        self.asr_downloading_models.insert(model);
        let sender = self.event_sender.clone();
        let url = model.download_url().to_string();
        let path = model.path();

        std::thread::spawn(move || {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }

            match Self::download_asr_with_progress(&url, &path, model, sender.clone()) {
                Ok(_) => {
                    let _ = sender.send(AppEvent::AsrDownloadComplete(model));
                }
                Err(e) => {
                    let _ = sender.send(AppEvent::AsrDownloadError(model, e));
                }
            }
        });
    }

    fn download_asr_with_progress(
        url: &str,
        path: &PathBuf,
        model: WhisperModelSize,
        sender: Sender<AppEvent>,
    ) -> Result<(), String> {
        use std::process::{Command, Stdio};
        use std::thread;
        use std::time::Duration;

        let path_str = path.to_string_lossy().to_string();
        let url = url.to_string();
        let expected_size = model.size_mb() * 1024 * 1024;

        let _ = sender.send(AppEvent::AsrDownloadProgress(model, 0.0));

        let has_wget = Command::new("wget").arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok();
        let mut child = if has_wget {
            let mut c = Command::new("wget");
            c.args([&url, "-O", &path_str, "--timeout=60", "--tries=3", "-q"])
             .stdout(Stdio::null())
             .stderr(Stdio::null())
             .spawn()
             .map_err(|e| format!("启动wget失败: {}", e))?
        } else if Command::new("curl").arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok() {
            let mut c = Command::new("curl");
            c.args(["-L", "-o", &path_str, &url, "--connect-timeout", "60", "--max-time", "600", "-s"])
             .stdout(Stdio::null())
             .stderr(Stdio::null())
             .spawn()
             .map_err(|e| format!("启动curl失败: {}", e))?
        } else {
            return Err("未找到wget或curl，请手动安装".to_string());
        };

        let path_clone = path.clone();
        let sender_clone = sender.clone();
        let progress_handle = thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_millis(500));
                if let Ok(metadata) = std::fs::metadata(&path_clone) {
                    let downloaded = metadata.len();
                    let percent = (downloaded as f64 / expected_size as f64 * 100.0).min(99.0);
                    let _ = sender_clone.send(AppEvent::AsrDownloadProgress(model, percent as f32));
                }
            }
        });

        let result = child.wait()
            .map_err(|e| format!("等待下载失败: {}", e))?;

        drop(progress_handle);

        if result.success() {
            let _ = sender.send(AppEvent::AsrDownloadProgress(model, 100.0));
            Ok(())
        } else {
            Err("下载失败".to_string())
        }
    }

    fn load_asr_model(&mut self, model: WhisperModelSize) {
        let model_path = model.path();
        if !model_path.exists() {
            self.asr_status = format!("{} 未下载", model.name());
            return;
        }

        // Check if already loaded
        if self.asr_sessions.contains_key(&model) {
            self.asr_status = format!("{} 已在运行", model.name());
            return;
        }

        self.asr_status = format!("正在加载 {} 模型...", model.name());

        match mofa_input::asr::AsrSession::new(&model_path) {
            Ok(session) => {
                self.asr_sessions.insert(model, Arc::new(Mutex::new(session)));
                // Initialize text box for this model if not exists
                self.asr_texts.entry(model).or_default();
                self.asr_status = format!("{} 已就绪", model.name());
            }
            Err(e) => {
                self.asr_status = format!("{} 加载失败: {}", model.name(), e);
            }
        }
    }

    fn unload_asr_model(&mut self, model: WhisperModelSize) {
        self.asr_sessions.remove(&model);
        self.asr_status = format!("{} 已卸载", model.name());
    }

    /// Resample audio from source rate to 16kHz (Whisper requires 16kHz)
    fn resample_audio(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
        if from_rate == to_rate {
            return samples.to_vec();
        }

        let ratio = to_rate as f64 / from_rate as f64;
        let new_len = (samples.len() as f64 * ratio) as usize;
        let mut result = Vec::with_capacity(new_len);

        for i in 0..new_len {
            let src_idx = i as f64 / ratio;
            let idx0 = src_idx.floor() as usize;
            let idx1 = (idx0 + 1).min(samples.len() - 1);
            let frac = src_idx - idx0 as f64;

            let val0 = samples[idx0] as f64;
            let val1 = samples[idx1] as f64;
            result.push((val0 + (val1 - val0) * frac) as f32);
        }

        result
    }

    fn start_recording(&mut self) {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

        if self.asr_sessions.is_empty() {
            self.asr_status = "错误: 请先加载语音模型".to_string();
            return;
        }

        let host = cpal::default_host();
        let device = match host.default_input_device() {
            Some(d) => d,
            None => {
                self.asr_status = "错误: 无麦克风".to_string();
                return;
            }
        };

        let config = match device.default_input_config() {
            Ok(c) => c,
            Err(e) => {
                self.asr_status = format!("错误: {}", e);
                return;
            }
        };

        let sample_rate = config.sample_rate().0;
        let channels = config.channels();
        let audio_samples_for_stream = self.audio_samples.clone();
        let audio_samples_for_process = self.audio_samples.clone();
        let sender = self.event_sender.clone();
        let asr_sessions: Vec<_> = self.asr_sessions.iter().map(|(k, v)| (*k, v.clone())).collect();

        // Create stop signal channel
        let (stop_tx, stop_rx) = std::sync::mpsc::channel::<()>();
        self.recording_stop_signal = Some(stop_tx);

        self.audio_samples.lock().unwrap().clear();
        self.is_recording = true;
        self.asr_status = "按住说话...".to_string();

        let handle = std::thread::spawn(move || {
            let err_fn = |err| eprintln!("音频错误: {}", err);

            let stream = match config.sample_format() {
                cpal::SampleFormat::F32 => {
                    device.build_input_stream(
                        &config.into(),
                        move |data: &[f32], _| {
                            let mut samples = audio_samples_for_stream.lock().unwrap();
                            if channels == 2 {
                                for chunk in data.chunks(2) {
                                    let mono = (chunk[0] + chunk[1]) / 2.0;
                                    samples.push(mono);
                                }
                            } else {
                                samples.extend_from_slice(data);
                            }
                        },
                        err_fn,
                        None,
                    )
                }
                _ => {
                    let _ = sender.send(AppEvent::AsrError("不支持的音频格式".to_string()));
                    return;
                }
            };

            if let Ok(stream) = stream {
                let _ = stream.play();

                // Wait for stop signal
                let _ = stop_rx.recv();

                drop(stream);

                // Get recorded samples
                let raw_samples = {
                    let s = audio_samples_for_process.lock().unwrap();
                    s.clone()
                };

                // Check if we have enough samples (at least 0.5 seconds at 16kHz)
                if raw_samples.len() < (sample_rate as usize / 2) {
                    let _ = sender.send(AppEvent::AsrError("录音太短，请按住更长时间".to_string()));
                    return;
                }

                // Resample to 16kHz (Whisper requirement)
                let samples = Self::resample_audio(&raw_samples, sample_rate, 16000);

                // Transcribe with all loaded models in parallel
                for (model, session) in asr_sessions {
                    let sender_clone = sender.clone();
                    let session_clone = session.clone();
                    let samples_clone = samples.clone();

                    std::thread::spawn(move || {
                        match session_clone.lock().unwrap().transcribe(&samples_clone) {
                            Ok(text) => {
                                let _ = sender_clone.send(AppEvent::AsrResult(model, text));
                            }
                            Err(e) => {
                                let _ = sender_clone.send(AppEvent::AsrModelError(model, format!("识别失败: {}", e)));
                            }
                        }
                    });
                }
            }
        });

        self.recording_thread_handle = Some(handle);
    }

    fn stop_recording(&mut self) {
        if !self.is_recording {
            return;
        }

        // Send stop signal
        if let Some(stop_tx) = self.recording_stop_signal.take() {
            let _ = stop_tx.send(());
        }

        self.is_recording = false;
        self.asr_status = "识别中...".to_string();

        // Wait for recording thread to finish
        if let Some(handle) = self.recording_thread_handle.take() {
            let _ = handle.join();
        }
    }

    fn load_model(&mut self) {
        let model_path = self.selected_model.path();
        if !model_path.exists() {
            self.status = format!("模型未下载");
            return;
        }

        self.is_loading = true;
        self.status = format!("正在加载 {} 模型...", self.selected_model.name());

        let sender = self.event_sender.clone();
        std::thread::spawn(move || {
            match mofa_input::llm::ChatSession::new(&model_path) {
                Ok(_) => {
                    let _ = sender.send(AppEvent::ModelLoaded);
                }
                Err(e) => {
                    let _ = sender.send(AppEvent::Error(e.to_string()));
                }
            }
        });
    }

    fn switch_model(&mut self, new_model: ModelSize) {
        if !new_model.path().exists() {
            self.download_model(new_model);
            return;
        }

        if self.chat.is_none() {
            self.selected_model = new_model;
            self.load_model();
            return;
        }

        if self.loaded_model == Some(new_model) {
            self.status = format!("{} 已在运行", new_model.name());
            return;
        }

        if !self.messages.is_empty() {
            self.pending_model = Some(new_model);
            self.show_switch_confirm = true;
        } else {
            self.selected_model = new_model;
            self.chat = None;
            self.loaded_model = None;
            self.token_count = 0;
            self.load_model();
        }
    }

    fn confirm_switch(&mut self) {
        if let Some(new_model) = self.pending_model {
            self.selected_model = new_model;
            self.chat = None;
            self.loaded_model = None;
            self.messages.clear();
            self.token_count = 0;
            self.show_switch_confirm = false;
            self.pending_model = None;
            self.load_model();
        }
    }

    fn cancel_switch(&mut self) {
        self.show_switch_confirm = false;
        self.pending_model = None;
        if let Some(loaded) = self.loaded_model {
            self.selected_model = loaded;
        }
    }

    fn send_message(&mut self) {
        if self.input.trim().is_empty() || self.chat.is_none() || self.is_generating {
            return;
        }

        let message = self.input.trim().to_string();
        self.input.clear();

        self.messages.push(ChatMessage {
            role: "user".to_string(),
            content: message.clone(),
        });

        self.current_response = String::new();
        self.messages.push(ChatMessage {
            role: "assistant".to_string(),
            content: String::new(),
        });

        self.is_generating = true;
        self.status = "生成中...".to_string();

        let chat = self.chat.clone().unwrap();
        let sender = self.event_sender.clone();

        std::thread::spawn(move || {
            let sender2 = sender.clone();
            chat.send_stream(&message, 512, 0.7, move |token| {
                let _ = sender2.send(AppEvent::Token(token.to_string()));
            });
            let _ = sender.send(AppEvent::GenerationComplete);
        });
    }

    fn clear_chat(&mut self) {
        if let Some(chat) = &self.chat {
            chat.clear();
        }
        self.messages.clear();
        self.token_count = 0;
        self.current_response.clear();
        self.status = "对话已清空".to_string();
    }

    fn handle_events(&mut self) {
        while let Ok(event) = self.event_receiver.try_recv() {
            match event {
                AppEvent::Token(token) => {
                    self.current_response.push_str(&token);
                    if let Some(last) = self.messages.last_mut() {
                        last.content = self.current_response.clone();
                    }
                }
                AppEvent::GenerationComplete => {
                    self.is_generating = false;
                    if let Some(chat) = &self.chat {
                        self.token_count = chat.token_count();
                    }
                    self.status = format!("就绪 ({} tokens)", self.token_count);
                }
                AppEvent::ModelLoaded => {
                    let model_path = self.selected_model.path();
                    self.chat = mofa_input::llm::ChatSession::new(&model_path).ok();
                    self.loaded_model = Some(self.selected_model);
                    self.is_loading = false;
                    self.status = format!("{} 已就绪", self.selected_model.name());
                }
                AppEvent::Error(e) => {
                    self.is_loading = false;
                    self.status = format!("错误: {}", e);
                }
                AppEvent::DownloadProgress(model, percent) => {
                    self.download_progress.insert(model, percent);
                    self.status = format!("{} 下载中... {:.1}%", model.name(), percent);
                }
                AppEvent::DownloadComplete(model) => {
                    self.downloading_models.remove(&model);
                    self.download_progress.remove(&model);
                    self.status = format!("{} 下载完成，点击加载", model.name());
                }
                AppEvent::DownloadError(model, e) => {
                    self.downloading_models.remove(&model);
                    self.status = format!("{} 下载失败: {}", model.name(), e);
                }
                // ASR events
                AppEvent::AsrDownloadProgress(model, percent) => {
                    self.asr_download_progress.insert(model, percent);
                    self.asr_status = format!("{} 下载中... {:.1}%", model.name(), percent);
                }
                AppEvent::AsrDownloadComplete(model) => {
                    self.asr_downloading_models.remove(&model);
                    self.asr_download_progress.remove(&model);
                    self.asr_status = format!("{} 下载完成，请手动加载", model.name());
                }
                AppEvent::AsrDownloadError(model, e) => {
                    self.asr_downloading_models.remove(&model);
                    self.asr_status = format!("{} 下载失败: {}", model.name(), e);
                }
                AppEvent::AsrResult(model, text) => {
                    self.is_recording = false;
                    self.asr_status = format!("{} 识别完成", model.name());
                    // Store result in model-specific text box
                    self.asr_texts.insert(model, text);
                }
                AppEvent::AsrModelError(model, e) => {
                    self.is_recording = false;
                    self.asr_status = format!("{} {}", model.name(), e);
                }
                AppEvent::AsrError(e) => {
                    self.is_recording = false;
                    self.asr_status = e;
                }
            }
        }
    }
}

impl eframe::App for ChatApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_events();

        if self.is_generating {
            ctx.request_repaint();
        }

        // Model switch confirmation
        if self.show_switch_confirm {
            egui::Window::new("切换模型")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(format!(
                        "切换到 {} 将清空当前对话。\n是否继续？",
                        self.pending_model.map(|m| m.name()).unwrap_or("")
                    ));
                    ui.horizontal(|ui| {
                        if ui.button("确认").clicked() {
                            self.confirm_switch();
                        }
                        if ui.button("取消").clicked() {
                            self.cancel_switch();
                        }
                    });
                });
        }

        // Delete model confirmation
        if self.show_delete_confirm {
            egui::Window::new("删除模型")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(format!(
                        "确认删除 {} 模型？\n此操作不可恢复。",
                        self.pending_delete.map(|m| m.name()).unwrap_or("")
                    ));
                    ui.horizontal(|ui| {
                        if ui.button("确认删除").clicked() {
                            if let Some(model) = self.pending_delete {
                                self.delete_model(model);
                            }
                            self.show_delete_confirm = false;
                            self.pending_delete = None;
                        }
                        if ui.button("取消").clicked() {
                            self.show_delete_confirm = false;
                            self.pending_delete = None;
                        }
                    });
                });
        }

        // ASR model manager window
        if self.show_asr_manager {
            egui::Window::new("语音模型管理")
                .collapsible(false)
                .resizable(true)
                .default_size([500.0, 600.0])
                .show(ctx, |ui| {
                    ui.label("选择 Whisper 语音模型:");
                    ui.separator();

                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for model in WhisperModelSize::all() {
                            let available = self.is_asr_model_available(model);
                            let downloading = self.asr_downloading_models.contains(&model);
                            let is_loaded = self.asr_sessions.contains_key(&model);

                            ui.horizontal(|ui| {
                                ui.strong(model.name());
                                ui.label(model.description());
                            });

                            ui.horizontal(|ui| {
                                if downloading {
                                    if let Some(&progress) = self.asr_download_progress.get(&model) {
                                        let progress_bar = egui::ProgressBar::new(progress / 100.0)
                                            .text(format!("{:.1}%", progress))
                                            .desired_height(20.0)
                                            .desired_width(150.0);
                                        ui.add(progress_bar);
                                    } else {
                                        ui.spinner();
                                        ui.label("准备下载...");
                                    }
                                } else if is_loaded {
                                    ui.colored_label(egui::Color32::GREEN, "● 运行中");
                                } else if available {
                                    ui.colored_label(egui::Color32::GREEN, "✓ 已下载");
                                } else {
                                    ui.colored_label(egui::Color32::RED, "✗ 未下载");
                                }

                                // Action buttons
                                if !downloading {
                                    if !available {
                                        if ui.button("下载").clicked() {
                                            self.download_asr_model(model);
                                        }
                                    } else if is_loaded {
                                        // Unload button
                                        if ui.button("停止").clicked() {
                                            self.unload_asr_model(model);
                                        }
                                    } else {
                                        if ui.button("加载").clicked() {
                                            self.load_asr_model(model);
                                        }
                                        let delete_btn = egui::Button::new("🗑 删除")
                                            .fill(egui::Color32::from_rgb(239, 68, 68));
                                        if ui.add(delete_btn).clicked() {
                                            let path = model.path();
                                            if path.exists() {
                                                let _ = std::fs::remove_file(&path);
                                            }
                                            self.asr_status = format!("{} 已删除", model.name());
                                        }
                                    }
                                }
                            });

                            // Show text box for loaded models
                            if is_loaded {
                                let text = self.asr_texts.entry(model).or_default();
                                ui.label("识别结果:");
                                let _response = egui::TextEdit::multiline(text)
                                    .desired_rows(3)
                                    .desired_width(ui.available_width())
                                    .show(ui);

                                ui.horizontal(|ui| {
                                    if ui.button("📋 复制").clicked() {
                                        ui.output_mut(|o| o.copied_text.clone_from(text));
                                    }
                                    if ui.button("📤 发送到LLM").clicked() && !text.is_empty() {
                                        self.input.clone_from(text);
                                    }
                                    if ui.button("🗑 清空").clicked() {
                                        text.clear();
                                    }
                                });
                            }

                            ui.separator();
                        }
                    });

                    ui.separator();
                    ui.label("使用说明:");
                    ui.label("1. 下载并加载一个或多个语音模型");
                    ui.label("2. 点击 🎤 录音按钮进行语音输入");
                    ui.label("3. 录音 5 秒后，所有加载的模型会同时识别");
                    ui.label("4. 各模型的识别结果会显示在对应文本框中");

                    if ui.button("关闭").clicked() {
                        self.show_asr_manager = false;
                    }
                });
        }

        // Download manager window
        if self.show_download_manager {
            egui::Window::new("模型管理")
                .collapsible(false)
                .resizable(true)
                .default_size([400.0, 300.0])
                .show(ctx, |ui| {
                    ui.label("模型存储位置: ~/.mofa/models/");
                    ui.separator();

                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for model in ModelSize::all() {
                            let available = self.is_model_available(model);
                            let downloading = self.downloading_models.contains(&model);

                            ui.horizontal(|ui| {
                                ui.strong(model.name());
                                ui.label(model.description());
                            });

                            ui.horizontal(|ui| {
                                if downloading {
                                    // Downloading - show progress and cancel button
                                    if let Some(&progress) = self.download_progress.get(&model) {
                                        let progress_bar = egui::ProgressBar::new(progress / 100.0)
                                            .text(format!("{:.1}%", progress))
                                            .desired_height(20.0)
                                            .desired_width(150.0);
                                        ui.add(progress_bar);
                                    } else {
                                        ui.spinner();
                                        ui.label("准备下载...");
                                    }
                                    let cancel_btn = egui::Button::new("取消")
                                        .fill(egui::Color32::from_rgb(239, 68, 68));
                                    if ui.add(cancel_btn).clicked() {
                                        self.cancel_download(model);
                                    }
                                } else if available {
                                    // Downloaded - show load/delete buttons
                                    ui.colored_label(egui::Color32::GREEN, "✓ 已下载");
                                    if self.loaded_model == Some(model) {
                                        ui.colored_label(egui::Color32::GREEN, "● 运行中");
                                        if ui.button("🗑 删除").clicked() {
                                            self.pending_delete = Some(model);
                                            self.show_delete_confirm = true;
                                        }
                                    } else {
                                        if ui.button("加载").clicked() {
                                            self.switch_model(model);
                                            self.show_download_manager = false;
                                        }
                                        let delete_btn = egui::Button::new("🗑 删除")
                                            .fill(egui::Color32::from_rgb(239, 68, 68));
                                        if ui.add(delete_btn).clicked() {
                                            self.pending_delete = Some(model);
                                            self.show_delete_confirm = true;
                                        }
                                    }
                                } else {
                                    ui.colored_label(egui::Color32::RED, "✗ 未下载");
                                    if ui.button("下载").clicked() {
                                        self.download_model(model);
                                    }
                                }
                            });

                            ui.separator();
                        }
                    });

                    if ui.button("关闭").clicked() {
                        self.show_download_manager = false;
                    }
                });
        }

        // Top panel
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Quick model buttons
                for model in ModelSize::all() {
                    let available = self.is_model_available(model);
                    let is_loaded = self.loaded_model == Some(model);
                    let downloading = self.downloading_models.contains(&model);

                    let btn_text = if downloading {
                        format!("{} ⏳", model.name())
                    } else if is_loaded {
                        format!("{} ●", model.name())
                    } else if available {
                        model.name().to_string()
                    } else {
                        format!("{} ✗", model.name())
                    };

                    let btn = if is_loaded {
                        egui::Button::new(&btn_text)
                            .fill(egui::Color32::from_rgb(34, 197, 94))
                    } else if !available {
                        egui::Button::new(&btn_text)
                            .fill(egui::Color32::from_rgb(239, 68, 68))
                    } else {
                        egui::Button::new(&btn_text)
                    };

                    if ui.add(btn).clicked() && !self.is_loading && !self.is_generating && !downloading {
                        if !available {
                            self.download_model(model);
                        } else {
                            self.switch_model(model);
                        }
                    }
                }

                ui.separator();

                if ui.button("模型管理").clicked() {
                    self.show_download_manager = true;
                }

                ui.separator();

                // ASR controls
                if ui.button("🎤 语音设置").clicked() {
                    self.show_asr_manager = true;
                }

                // Microphone button for hold-to-record
                let mic_btn_text = if self.is_recording {
                    "🔴 录音中... (松开结束)"
                } else {
                    "🎤 按住说话"
                };
                let mic_btn_color = if self.is_recording {
                    egui::Color32::from_rgb(239, 68, 68)
                } else {
                    egui::Color32::from_rgb(59, 130, 246)
                };
                // Use drag sense to detect hold
                let mic_btn = egui::Button::new(mic_btn_text)
                    .fill(mic_btn_color)
                    .sense(egui::Sense::drag());

                let mic_response = ui.add(mic_btn);

                // Handle press and release for hold-to-record
                if !self.asr_sessions.is_empty() {
                    // Start recording when drag starts (mouse pressed)
                    if mic_response.drag_started() && !self.is_recording {
                        self.start_recording();
                    }
                    // Stop recording when drag released (mouse released)
                    if mic_response.drag_released() && self.is_recording {
                        self.stop_recording();
                    }
                }

                // Show ASR status
                if !self.asr_status.is_empty() {
                    ui.label(&self.asr_status);
                }

                // Show download progress for active downloads
                if !self.downloading_models.is_empty() {
                    ui.separator();
                    for model in ModelSize::all() {
                        if self.downloading_models.contains(&model) {
                            ui.vertical(|ui| {
                                ui.set_width(120.0);
                                let progress = self.download_progress.get(&model).copied().unwrap_or(0.0);
                                ui.add(
                                    egui::ProgressBar::new(progress / 100.0)
                                        .text(format!("{} {:.0}%", model.name(), progress))
                                        .desired_height(16.0)
                                );
                            });
                        }
                    }
                }

                if self.is_loading {
                    ui.spinner();
                }

                ui.separator();

                if ui.button("清空").clicked() {
                    self.clear_chat();
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(&self.status);
                });
            });
            ui.separator();
        });

        // Main chat area
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.chat.is_none() {
                ui.vertical_centered(|ui| {
                    ui.add_space(100.0);
                    ui.heading("👋 欢迎使用本地 LLM 聊天");
                    ui.add_space(20.0);
                    ui.label("点击上方模型按钮开始");
                    ui.add_space(10.0);
                    ui.label("绿色● = 运行中 | 红色✗ = 需下载 | ⏳ = 下载中");
                    ui.add_space(20.0);
                    ui.label("模型自动下载到: ~/.mofa/models/");
                });
            } else {
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for msg in &self.messages {
                            let (bg_color, name, text_color) = if msg.role == "user" {
                                (egui::Color32::from_rgb(59, 130, 246), "你", egui::Color32::WHITE)
                            } else {
                                (egui::Color32::from_rgb(31, 41, 55), "AI", egui::Color32::WHITE)
                            };

                            ui.label(egui::RichText::new(name).color(text_color).strong());

                            egui::Frame::group(ui.style())
                                .fill(bg_color)
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.label(egui::RichText::new(&msg.content).color(text_color).size(14.0));
                                });

                            ui.add_space(10.0);
                        }

                        if self.is_generating {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label("生成中...");
                            });
                        }
                    });
            }
        });

        // Bottom input panel
        egui::TopBottomPanel::bottom("input_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let available_width = ui.available_width();
                let text_edit = egui::TextEdit::multiline(&mut self.input)
                    .hint_text("输入消息... (Enter发送, Shift+Enter换行)")
                    .desired_rows(2)
                    .lock_focus(true);

                let response = ui.add_sized(
                    egui::vec2(available_width - 80.0, 60.0),
                    text_edit
                );

                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift) {
                    self.send_message();
                    response.request_focus();
                }

                ui.vertical(|ui| {
                    let send_btn = egui::Button::new("发送")
                        .fill(egui::Color32::from_rgb(59, 130, 246));
                    if ui.add_sized(egui::vec2(70.0, 28.0), send_btn).clicked() && !self.is_generating {
                        self.send_message();
                    }

                    if ui.add_sized(egui::vec2(70.0, 28.0), egui::Button::new("退出")).clicked() {
                        std::process::exit(0);
                    }
                });
            });
        });
    }
}

fn main() {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 700.0])
            .with_min_inner_size([600.0, 400.0]),
        ..Default::default()
    };

    eframe::run_native(
        "本地 LLM 聊天",
        options,
        Box::new(|cc| {
            // Configure Chinese font support
            let mut fonts = egui::FontDefinitions::default();

            let font_paths = [
                "/System/Library/Fonts/Hiragino Sans GB.ttc",
                "/System/Library/Fonts/STHeiti Light.ttc",
                "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
                "C:\\Windows\\Fonts\\msyh.ttc",
            ];

            for path in &font_paths {
                if let Ok(font_data) = std::fs::read(path) {
                    fonts.font_data.insert(
                        "chinese".to_owned(),
                        egui::FontData::from_owned(font_data),
                    );

                    if let Some(proportional) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
                        proportional.push("chinese".to_owned());
                    }
                    if let Some(monospace) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
                        monospace.push("chinese".to_owned());
                    }

                    cc.egui_ctx.set_fonts(fonts);
                    break;
                }
            }

            Box::new(ChatApp::new())
        }),
    ).unwrap();
}
