# UI 레이아웃: Split 모드 (2~4개 세션 동시 표시)

- phase: 1
- size: M
- blocked_by: 007-termesh-layout

## 목표
- tmux 스타일 Split 모드 UI 구현
- 2~4개 세션의 터미널을 동시에 표시

## 완료 기준
- [ ] Dual (1x2) 레이아웃
  - 화면을 세로로 2등분
- [ ] Quad (2x2) 레이아웃
  - 화면을 2x2 격자로 분할
- [ ] 각 pane의 우상단에 세션명 바
  - 세션명 + 상태 아이콘
- [ ] Cmd+H/J/K/L로 포커스 이동
- [ ] Cmd+Enter로 포커스된 pane 줌 (풀스크린)
- [ ] 다시 Cmd+Enter로 Split 모드로 복원
- [ ] 각 pane 경계에 divider 표시
- [ ] 화면 리사이징 시 모든 pane 비율 유지
- [ ] 통합 테스트
- [ ] `cargo test` 통과

## 포함할 모듈
- pane_grid: Pane 격자 구조체
- split_layout: 분할 레이아웃 관리
- divider: 경계선 렌더링

## 노트
- 사이드 패널 없음 (Focus 모드로 전환 필요)
- Pane 경계선: 밝은 회색 (#404040)
- Split 모드에서 Cmd+Enter → Focus 모드로 자동 전환
