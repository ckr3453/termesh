# Phase 4: 멀티세션 + 화면 분할 (완료)

완료일: 2026-03-01

## 태스크 요약

| # | 태스크 | 크기 | 요약 |
|---|--------|------|------|
| 033 | Action → SessionManager 연결 | M | 모든 Action을 실제 API 호출로 연결 (split/close/focus/navigate/zoom) |
| 034 | 멀티그리드 렌더링 | M | on_tick()에서 모든 visible pane의 grid를 layout 좌표로 반환 (033에서 함께 구현) |
| 035 | 패인 경계선 렌더링 | S | compute_dividers()로 1px 회색 경계선 렌더링, zoom 시 숨김 |
| 036 | 세션별 독립 PTY resize | S | pane.grid_size()로 개별 세션 resize, cell_w/cell_h 전달 |

## 주요 변경

- `termesh-app/main.rs`: SplitLayoutManager 통합, 모든 Action 실제 구현
- `termesh-platform/event_loop.rs`: should_exit, dividers, per-pane resize 콜백
- `termesh-renderer/renderer.rs`: render_grids에 divider 렌더링 패스 추가
- `termesh-layout/split_layout.rs`: layout_mut() 접근자 추가

## 커밋

- `fc2f610` feat: wire all actions to session manager and layout engine
- `2958046` feat: add pane border divider rendering
- `d2d2348` feat: per-pane PTY resize based on layout dimensions
