# termesh-layout: pane 분할 엔진 (quad/dual)

- phase: 1
- size: M
- blocked_by: 006-termesh-platform

## 목표
- 화면 분할 레이아웃 엔진
- Quad (2x2), Dual (1x2) 레이아웃 지원
- Pane 포커싱, 리사이징

## 완료 기준
- [ ] `Layout` enum: Quad, Dual, Single, Custom
- [ ] `Pane` 구조체: 위치, 크기, 세션 ID
- [ ] `split_horizontal()`, `split_vertical()` 구현
- [ ] `resize_pane()` 구현 (비율 기반)
- [ ] `focus_pane()` 구현 (선택)
- [ ] 레이아웃 저장/복원
- [ ] 단위 테스트
- [ ] `cargo test` 통과

## 포함할 모듈
- layout: 레이아웃 엔진
- pane: Pane 구조체
- splitter: 분할 로직

## 노트
- 레이아웃은 설정(`default_mode`, `split_layout`)에서 선택
- 탭 기능은 Phase 2에서
