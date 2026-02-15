fn model_base_dir() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".mofa/models"))
        .unwrap_or_else(|| PathBuf::from("./models"))
}

fn choose_llm_model(base: &Path, choice: LlmModelChoice) -> Option<PathBuf> {
    if let Some(file_name) = choice.file_name() {
        let selected = base.join(file_name);
        if selected.exists() {
            return Some(selected);
        }
    }
    choose_llm_model_auto(base)
}

fn choose_llm_model_auto(base: &Path) -> Option<PathBuf> {
    let mem_gb = total_memory_gb().unwrap_or(32);

    let preferred = if mem_gb <= 8 {
        "qwen2.5-0.5b-q4_k_m.gguf"
    } else if mem_gb <= 16 {
        "qwen2.5-1.5b-q4_k_m.gguf"
    } else if mem_gb <= 24 {
        "qwen2.5-3b-q4_k_m.gguf"
    } else if mem_gb <= 40 {
        "qwen3-4b-q4_k_m.gguf"
    } else if mem_gb <= 64 {
        "qwen2.5-7b-q4_k_m.gguf"
    } else if mem_gb <= 96 {
        "qwen3-8b-q4_k_m.gguf"
    } else if mem_gb <= 128 {
        "qwen3-14b-q4_k_m.gguf"
    } else if mem_gb <= 192 {
        "qwen3-30b-a3b-q4_k_m.gguf"
    } else if mem_gb <= 256 {
        "qwen3-32b-q4_k_m.gguf"
    } else {
        "qwen2.5-72b-q4_k_m.gguf"
    };

    let mut candidates = vec![
        preferred,
        "qwen2.5-1.5b-q4_k_m.gguf",
        "qwen2.5-0.5b-q4_k_m.gguf",
        "qwen2.5-3b-q4_k_m.gguf",
        "qwen3-4b-q4_k_m.gguf",
        "qwen2.5-7b-q4_k_m.gguf",
        "qwen3-8b-q4_k_m.gguf",
        "qwen2.5-14b-q4_k_m.gguf",
        "qwen3-14b-q4_k_m.gguf",
        "qwen3-30b-a3b-q4_k_m.gguf",
        "qwen2.5-32b-q4_k_m.gguf",
        "qwen3-32b-q4_k_m.gguf",
        "qwen2.5-72b-q4_k_m.gguf",
        "qwen2.5-coder-1.5b-q4_k_m.gguf",
        "qwen2.5-coder-0.5b-q4_k_m.gguf",
        "qwen2.5-coder-3b-q4_k_m.gguf",
        "qwen2.5-coder-7b-q4_k_m.gguf",
        "qwen2.5-coder-14b-q4_k_m.gguf",
        "qwen2.5-coder-32b-q4_k_m.gguf",
    ];
    candidates.dedup();

    candidates
        .into_iter()
        .map(|name| base.join(name))
        .find(|p| p.exists())
}

fn choose_asr_model(base: &Path, choice: AsrModelChoice) -> Option<PathBuf> {
    if let Some(file_name) = choice.file_name() {
        let selected = base.join(file_name);
        if selected.exists() {
            return Some(selected);
        }
    }
    choose_asr_model_auto(base)
}

fn choose_asr_model_auto(base: &Path) -> Option<PathBuf> {
    [
        "ggml-small.bin",
        "ggml-base.bin",
        "ggml-tiny.bin",
        "ggml-medium.bin",
    ]
    .into_iter()
    .map(|name| base.join(name))
    .find(|p| p.exists())
}

fn normalize_transcript(text: &str) -> String {
    let mut out = String::new();
    let mut prev_space = false;
    for ch in text.trim().chars() {
        if ch.is_whitespace() {
            if !prev_space {
                out.push(' ');
            }
            prev_space = true;
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    out.trim().to_string()
}

fn audio_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let mean_square = samples
        .iter()
        .map(|v| {
            let x = *v as f64;
            x * x
        })
        .sum::<f64>()
        / samples.len() as f64;
    mean_square.sqrt() as f32
}

fn build_refine_prompt(raw_text: &str) -> String {
    format!(
        "你是输入法润色器。将 ASR 文本整理为可直接发送的自然表达。\n\
规则：\n\
1) 保留原意与事实，不新增信息；\n\
2) 删除重复、卡顿与明显口吃；语气词与语气助词仅在原文已有且承载语义时保留，不得自行新增句末“呀/呢”；\n\
3) 专名、数字、代码、URL 原样保留；\n\
4) 若原文含英文/中英混合，尽量保留英文词形、大小写与常见短语，不强制翻译为中文；\n\
5) 若存在明显 ASR 误识（同音误字、语境不通），可基于上下文做最小必要纠正；若不确定，保留原词，不要臆造；\n\
6) 优先贴近用户原始说话方式：保留原句式、措辞与语气强弱，不要强行“职业化”“官方化”或套用固定人设口吻；\n\
7) 若原文本无技术词，不要硬加；若原文有技术词，按原习惯保留，不做生硬替换；\n\
8) 可做轻微顺句与标点修复，但总体风格应平实克制，像“用户本人说的话”；\n\
9) 若原文句末无“呀/呢”，输出句末也不要新增“呀/呢”；\n\
10) 若内容确为空，输出空字符串；\n\
11) 只输出最终文本，不解释、不提问。\n\n{}",
        raw_text
    )
}

fn should_skip_llm_refine(raw_text: &str) -> bool {
    let t = raw_text.trim();
    if t.is_empty() {
        return true;
    }

    // Skip LLM for full English paragraphs/sentences to avoid unwanted rewriting.
    let mut english_letters = 0usize;
    let mut cjk_chars = 0usize;
    for ch in t.chars() {
        if ch.is_ascii_alphabetic() {
            english_letters += 1;
        } else if ('\u{4E00}'..='\u{9FFF}').contains(&ch) {
            cjk_chars += 1;
        }
    }
    let total_lang = english_letters + cjk_chars;
    if total_lang == 0 {
        return false;
    }

    let english_ratio = english_letters as f32 / total_lang as f32;
    english_letters >= 16 && english_ratio >= 0.9
}

fn has_terminal_punctuation(text: &str) -> bool {
    match text.trim_end().chars().last() {
        Some(ch) => matches!(ch, '。' | '！' | '？' | '.' | '!' | '?' | '…'),
        None => false,
    }
}

fn trim_added_terminal_period(raw_text: &str, refined_text: &str) -> String {
    fn strip_trailing_punct(s: &str) -> (&str, &str) {
        let mut cut = s.len();
        for (idx, ch) in s.char_indices().rev() {
            if ch.is_whitespace() {
                cut = idx;
                continue;
            }
            if matches!(ch, '。' | '！' | '？' | '.' | '!' | '?' | '…') {
                cut = idx;
                continue;
            }
            break;
        }
        s.split_at(cut)
    }

    let mut out = refined_text.trim().to_string();

    // Keep user's no-period style: if raw has no terminal punctuation, strip added period.
    if !has_terminal_punctuation(raw_text) {
        while out.ends_with('。') || out.ends_with('.') {
            out.pop();
            out = out.trim_end().to_string();
        }
    }

    // Forbid adding terminal "呀/呢" when raw does not end with them.
    let raw_core = strip_trailing_punct(raw_text.trim()).0.trim_end();
    let raw_tail = raw_core.chars().last();
    let raw_has_particle = matches!(raw_tail, Some('呀' | '呢'));
    if !raw_has_particle {
        let (core, punct) = strip_trailing_punct(out.trim());
        let mut core_owned = core.trim_end().to_string();
        if matches!(core_owned.chars().last(), Some('呀' | '呢')) {
            core_owned.pop();
            core_owned = core_owned.trim_end().to_string();
            out = if punct.is_empty() {
                core_owned
            } else {
                format!("{core_owned}{punct}")
            };
        }
    }

    out
}

fn total_memory_gb() -> Option<u64> {
    let name = CString::new("hw.memsize").ok()?;
    let mut value: u64 = 0;
    let mut size = std::mem::size_of::<u64>();
    let ret = unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            &mut value as *mut _ as *mut c_void,
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    if ret == 0 {
        Some(value / 1024 / 1024 / 1024)
    } else {
        None
    }
}
