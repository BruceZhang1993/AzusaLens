use std::cell::Cell;

use azusa_config::LanguageMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EffectiveLanguage {
    English,
    SimplifiedChinese,
}

thread_local! {
    static CURRENT_LANGUAGE: Cell<EffectiveLanguage> = const { Cell::new(EffectiveLanguage::English) };
}

pub(crate) fn apply_language(mode: LanguageMode) -> Result<EffectiveLanguage, String> {
    let language = effective_language(mode);
    let bundled = match language {
        EffectiveLanguage::English => "",
        EffectiveLanguage::SimplifiedChinese => "zh-CN",
    };
    slint::select_bundled_translation(bundled).map_err(|error| error.to_string())?;
    CURRENT_LANGUAGE.with(|current| current.set(language));
    Ok(language)
}

#[must_use]
pub(crate) fn current_language() -> EffectiveLanguage {
    CURRENT_LANGUAGE.with(Cell::get)
}

#[must_use]
pub(crate) fn effective_language(mode: LanguageMode) -> EffectiveLanguage {
    match mode {
        LanguageMode::English => EffectiveLanguage::English,
        LanguageMode::SimplifiedChinese => EffectiveLanguage::SimplifiedChinese,
        LanguageMode::System => system_language(),
    }
}

fn system_language() -> EffectiveLanguage {
    let locale = sys_locale::get_locale()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if locale.starts_with("zh") {
        EffectiveLanguage::SimplifiedChinese
    } else {
        EffectiveLanguage::English
    }
}

#[must_use]
pub(crate) fn localize_status(raw: &str) -> String {
    if current_language() == EffectiveLanguage::English || raw.is_empty() {
        return raw.to_owned();
    }

    if let Some(value) = exact_status(raw) {
        return value.to_owned();
    }

    if let Some(value) = localized_suffix(
        raw,
        "Settings loaded with safe defaults · ",
        "设置已使用安全默认值加载 · ",
    ) {
        return value;
    }
    if let Some(value) =
        localized_suffix(raw, "Unsupported appearance mode · ", "不支持的外观模式 · ")
    {
        return value;
    }
    if let Some(value) =
        localized_suffix(raw, "Unsupported language mode · ", "不支持的语言模式 · ")
    {
        return value;
    }
    if let Some(value) = localized_suffix(raw, "Could not apply language · ", "无法应用语言设置 · ")
    {
        return value;
    }
    if let Some(value) = localized_suffix(raw, "Could not save language · ", "无法保存语言设置 · ")
    {
        return value;
    }
    if let Some(value) = raw.strip_prefix("Appearance saved · ") {
        return format!("外观已保存 · {}", localize_mode(value));
    }
    if let Some(value) =
        localized_suffix(raw, "Could not save appearance · ", "无法保存外观设置 · ")
    {
        return value;
    }
    if let Some(value) = localized_suffix(
        raw,
        "Default export directory saved · ",
        "快速保存目录已保存 · ",
    ) {
        return value;
    }
    if let Some(value) = localized_suffix(
        raw,
        "Could not save export directory · ",
        "无法保存快速保存目录 · ",
    ) {
        return value;
    }
    if let Some(value) = localized_suffix(
        raw,
        "Could not choose export directory · ",
        "无法选择快速保存目录 · ",
    ) {
        return value;
    }
    if let Some(value) = localized_suffix(
        raw,
        "Could not reset export directory · ",
        "无法重置快速保存目录 · ",
    ) {
        return value;
    }
    if let Some(value) = localized_suffix(
        raw,
        "Could not save export preferences · ",
        "无法保存导出设置 · ",
    ) {
        return value;
    }
    if let Some(value) = localized_suffix(
        raw,
        "Region capture worker failed · ",
        "区域截图任务启动失败 · ",
    ) {
        return value;
    }
    if let Some(value) = localized_suffix(
        raw,
        "Selection overlay failed · ",
        "截图选择浮层打开失败 · ",
    ) {
        return value;
    }
    if let Some(value) = localized_suffix(raw, "Region capture failed · ", "区域截图失败 · ")
    {
        return value;
    }
    if let Some(value) = localized_suffix(raw, "Region crop failed · ", "截图裁剪失败 · ") {
        return value;
    }
    if let Some(value) = raw.strip_prefix("Annotation tool · ") {
        return format!("标注工具 · {}", localize_tool(value));
    }
    if let Some(value) = localized_suffix(raw, "Color update failed · ", "颜色更新失败 · ")
    {
        return value;
    }
    if let Some(value) = localized_suffix(raw, "Size update failed · ", "尺寸更新失败 · ") {
        return value;
    }
    if let Some(value) = localized_suffix(raw, "Annotation preview failed · ", "标注预览失败 · ")
    {
        return value;
    }
    if let Some(value) = localized_suffix(raw, "Annotation failed · ", "标注失败 · ") {
        return value;
    }
    if let Some(value) = localized_suffix(raw, "Text annotation failed · ", "文字标注失败 · ")
    {
        return value;
    }
    if let Some(value) = localized_suffix(raw, "Undo failed · ", "撤销失败 · ") {
        return value;
    }
    if let Some(value) = localized_suffix(raw, "Redo failed · ", "重做失败 · ") {
        return value;
    }
    if let Some(value) = localized_suffix(raw, "Delete failed · ", "删除失败 · ") {
        return value;
    }
    if let Some(value) = localized_suffix(raw, "Clear failed · ", "清空失败 · ") {
        return value;
    }
    if let Some(value) = localized_suffix(raw, "Clipboard failed · ", "剪贴板操作失败 · ")
    {
        return value;
    }
    if let Some(value) = localized_suffix(raw, "Quick save failed · ", "快速保存失败 · ") {
        return value;
    }
    if let Some(value) = localized_suffix(raw, "Saved edited PNG · ", "已保存编辑后的 PNG · ")
    {
        return value;
    }
    if let Some(value) = localized_suffix(raw, "Save failed · ", "保存失败 · ") {
        return value;
    }
    if let Some(value) = localized_suffix(raw, "Could not open Save As · ", "无法打开另存为窗口 · ")
    {
        return value;
    }
    if let Some(value) = localized_suffix(raw, "Saved As · ", "已另存为 · ") {
        return value;
    }
    if let Some(value) = localized_suffix(raw, "Save As failed · ", "另存为失败 · ") {
        return value;
    }
    if let Some(value) = localized_suffix(raw, "OCR input failed · ", "OCR 输入处理失败 · ")
    {
        return value;
    }
    if let Some(value) = localized_suffix(raw, "OCR worker unavailable · ", "OCR 工作线程不可用 · ")
    {
        return value;
    }
    if let Some(value) = raw.strip_suffix(" running locally…") {
        return format!("{value} 正在本地识别…");
    }
    if let Some(value) = localized_suffix(
        raw,
        "OCR model worker unavailable · ",
        "OCR 模型工作线程不可用 · ",
    ) {
        return value;
    }
    if let Some(value) = raw
        .strip_prefix("Starting ")
        .and_then(|value| value.strip_suffix(" download…"))
    {
        return format!("正在开始下载 {value}…");
    }
    if let Some(value) = raw
        .strip_prefix("Downloading ")
        .and_then(|value| value.strip_suffix('…'))
    {
        return format!("正在下载 {value}…");
    }
    if raw == "Cancelling OCR model download…" {
        return "正在取消 OCR 模型下载…".to_owned();
    }
    if let Some(value) = raw.strip_prefix("Enabled OCR model · ") {
        return format!("已启用 OCR 模型 · {value}");
    }
    if let Some(value) =
        localized_suffix(raw, "Could not enable OCR model · ", "无法启用 OCR 模型 · ")
    {
        return value;
    }
    if let Some(value) = raw
        .strip_prefix("Removing ")
        .and_then(|value| value.strip_suffix('…'))
    {
        return format!("正在删除 {value}…");
    }
    if let Some(value) = raw
        .strip_prefix("Local OCR completed · ")
        .and_then(|value| value.strip_suffix(" text blocks · OCR text layer is not exported"))
    {
        return format!("本地 OCR 已完成 · {value} 个文字区域 · OCR 文字层不会导出到图片");
    }
    if let Some(value) = localized_suffix(raw, "Local OCR failed · ", "本地 OCR 失败 · ") {
        return value;
    }
    if let Some(value) = raw
        .strip_prefix("Downloaded ")
        .and_then(|value| value.strip_suffix(" · select Enable to use it for OCR"))
    {
        return format!("已下载 {value} · 选择“启用”后即可用于 OCR");
    }
    if let Some(value) = raw.strip_prefix("Removed OCR model · ") {
        return format!("已删除 OCR 模型 · {value}");
    }
    if let Some(value) = raw
        .strip_prefix("Download cancelled · ")
        .and_then(|value| value.strip_suffix(" remains disabled"))
    {
        return format!("下载已取消 · {value} 保持未启用状态");
    }
    if let Some(value) =
        localized_suffix(raw, "OCR model download failed · ", "OCR 模型下载失败 · ")
    {
        return value;
    }
    if let Some(value) = localized_suffix(raw, "OCR model removal failed · ", "OCR 模型删除失败 · ")
    {
        return value;
    }
    if let Some(value) = raw.strip_prefix("Captured ") {
        if let Some(dimensions) = value.strip_suffix(" · copied to clipboard · ready to annotate")
        {
            return format!("已截图 {dimensions} · 已复制到剪贴板 · 可以开始标注");
        }
        if let Some((dimensions, error)) = value.split_once(" · clipboard failed: ")
            && let Some(error) = error.strip_suffix(" · ready to annotate")
        {
            return format!("已截图 {dimensions} · 复制到剪贴板失败：{error} · 可以开始标注");
        }
        if let Some(dimensions) = value.strip_suffix(" · ready to annotate") {
            return format!("已截图 {dimensions} · 可以开始标注");
        }
    }
    if let Some(value) = raw.strip_prefix("Saved shortcut is invalid · ")
        && let Some(error) = value.strip_suffix(" · using PrtSc until it is changed")
    {
        return format!("已保存的快捷键无效 · {error} · 修改前暂时使用 PrtSc");
    }
    if let Some(value) = localized_suffix(raw, "Shortcut rejected · ", "快捷键被拒绝 · ") {
        return value;
    }
    if let Some(value) = localized_suffix(raw, "Could not save shortcut · ", "无法保存快捷键 · ")
    {
        return value;
    }
    if let Some(value) = raw.strip_prefix("Previous shortcut restored · ") {
        return format!("已恢复上一个快捷键 · {value}");
    }
    if let Some(value) = raw.strip_prefix("Screenshot shortcut active · ") {
        return format!("截图快捷键已生效 · {value}");
    }
    if let Some(value) = raw.strip_prefix("Registering screenshot shortcut · ") {
        return format!("正在注册截图快捷键 · {value}");
    }
    if let Some(value) = raw
        .strip_prefix("Registering ")
        .and_then(|value| value.strip_suffix('…'))
    {
        return format!("正在注册 {value}…");
    }

    raw.to_owned()
}

fn exact_status(raw: &str) -> Option<&'static str> {
    Some(match raw {
        "Ready · capture a region, then use the overlay tools" => {
            "就绪 · 截取区域后即可使用浮层工具"
        }
        "Language saved" => "语言设置已保存",
        "Export directory unchanged" => "快速保存目录未更改",
        "Default export directory reset" => "快速保存目录已重置",
        "Export preferences saved" => "导出设置已保存",
        "Starting region capture…" => "正在开始区域截图…",
        "Region capture cancelled" => "已取消区域截图",
        "Updated selected object color" => "已更新所选对象颜色",
        "Updated selected object size" => "已更新所选对象尺寸",
        "Text anchor placed · type text and choose Add text" => {
            "已放置文字锚点 · 输入文字后选择“添加文字”"
        }
        "Text annotation added" => "已添加文字标注",
        "Undid last editor change" => "已撤销上一次编辑",
        "Redid editor change" => "已重做编辑",
        "Deleted selected annotation" => "已删除所选标注",
        "All annotations cleared" => "已清空全部标注",
        "Nothing to copy · capture a region first" => "没有可复制的内容 · 请先截取区域",
        "Image copied" => "图片已复制",
        "Edited image copied to clipboard" => "编辑后的图片已复制到剪贴板",
        "Nothing to save · capture a region first" => "没有可保存的内容 · 请先截取区域",
        "Image saved" => "图片已保存",
        "Save As cancelled" => "已取消另存为",
        "Choose an OCR model to download and enable" => "请选择一个 OCR 模型下载并启用",
        "Nothing to OCR · capture a region first" => "没有可识别的内容 · 请先截取区域",
        "No OCR text to copy" => "没有可复制的 OCR 文字",
        "OCR text copied to the clipboard" => "OCR 文字已复制到剪贴板",
        "No OCR text is selected" => "尚未选择 OCR 文字",
        "Selected OCR text copied to the clipboard" => "所选 OCR 文字已复制到剪贴板",
        "Selected OCR text is empty" => "所选 OCR 文字为空",
        "Local OCR completed · no text found" => "本地 OCR 已完成 · 未识别到文字",
        "Unsupported shortcut key · choose another key" => "不支持此快捷键 · 请选择其他按键",
        "Shortcut is already active" => "该快捷键已经生效",
        "PrtSc is already the active shortcut" => "PrtSc 已经是当前快捷键",
        _ => return None,
    })
}

fn localized_suffix(raw: &str, english_prefix: &str, chinese_prefix: &str) -> Option<String> {
    raw.strip_prefix(english_prefix)
        .map(|value| format!("{chinese_prefix}{value}"))
}

fn localize_mode(value: &str) -> &str {
    match value {
        "system" => "跟随系统",
        "light" => "亮色",
        "dark" => "暗色",
        other => other,
    }
}

fn localize_tool(value: &str) -> &str {
    match value {
        "select" => "选择",
        "rectangle" => "矩形",
        "arrow" => "箭头",
        "pen" => "画笔",
        "text" => "文字",
        "mosaic" => "马赛克",
        "ellipse" => "椭圆",
        "line" => "直线",
        "number" => "序号",
        "blur" => "模糊",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_language_modes_resolve_without_system_locale() {
        assert_eq!(
            effective_language(LanguageMode::English),
            EffectiveLanguage::English
        );
        assert_eq!(
            effective_language(LanguageMode::SimplifiedChinese),
            EffectiveLanguage::SimplifiedChinese
        );
    }

    #[test]
    fn chinese_status_localizes_dynamic_capture_result() {
        CURRENT_LANGUAGE.with(|language| language.set(EffectiveLanguage::SimplifiedChinese));
        assert_eq!(
            localize_status("Captured 640×480 · copied to clipboard · ready to annotate"),
            "已截图 640×480 · 已复制到剪贴板 · 可以开始标注"
        );
        CURRENT_LANGUAGE.with(|language| language.set(EffectiveLanguage::English));
    }
}
