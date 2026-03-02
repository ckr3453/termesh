# Phase 5.6: 사이드 패널 개선 — 완료 요약

완료일: 2026-03-02

## Diff UX

### 035 — 변경 파일 목록 + 파일 선택 diff
- 사이드 패널에 변경 파일 목록 표시 (상태 문자 M/A + 파일명 + +N -N 통계)
- Primary+Shift+Up/Down으로 파일 선택, Enter로 diff 표시, Escape로 복귀
- initial snapshot vs current 누적 diff (revert 감지)
- 변경 파일: history.rs, ui_grid.rs, main.rs, action.rs, keymap.rs, handler.rs

### 036 — Unified / Side-by-side diff 모드 전환
- `DiffMode::Unified` / `DiffMode::SideBySide` enum
- Side-by-side: 좌측(삭제) / 우측(추가) 분할 + │ 디바이더
- Primary+Shift+D로 모드 전환
- Equal 라인 양쪽 동일 표시, Insert/Delete paired alignment
- 변경 파일: diff_generator.rs, ui_grid.rs, main.rs, action.rs, keymap.rs, handler.rs, history.rs

## 렌더링 품질

### 037 — MSDF 폰트 렌더링 도입
- fontdue bitmap → fdsm 0.8 MSDF 전환 (해상도 독립적 텍스트)
- Primary font (CascadiaMono): 48×48 MSDF 아틀라스, Affine2 변환
- Fallback font (CJK/emoji): 기존 bitmap 유지
- WGSL 셰이더: median3 + screen_px_range MSDF 샘플링
- Atlas sampler: Nearest → Linear (MSDF 필수)
- 변경 파일: font.rs, glyph_cache.rs, terminal.wgsl, renderer.rs, Cargo.toml
