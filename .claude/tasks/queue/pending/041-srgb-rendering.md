# 041: sRGB 색공간 정리 + 렌더링 고도화

**Phase**: 8 - 기본 터미널 완성도
**Priority**: 낮음
**Difficulty**: 중간
**Blocked by**: 040

## 목표
색공간 설정 명확화, 향후 서브픽셀/COLR 지원 기반 마련.

## 작업 내용

### sRGB 색공간
1. renderer.rs:144의 non-sRGB 선택 이유 조사
2. sRGB로 전환 시 색상 차이 확인 (ANSI 256색, truecolor)
3. 의도적 선택이면 주석 문서화, 아니면 sRGB로 전환

### 서브픽셀 렌더링
- glyphon 0.10 지원 범위 확인
- Retina 디스플레이에서 선명도 비교

### Color emoji COLR
- glyphon의 COLR/SVG glyph 지원 현황 확인
- 미지원 시 workaround 또는 업스트림 대기

## 검증
- ANSI 색상 테스트 (256color 스크립트)
- Retina에서 텍스트 선명도 비교
- `cargo test --all` 통과
