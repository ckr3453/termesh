# termesh-input: 키바인딩 엔진

- phase: 1
- size: M
- blocked_by: 006-termesh-platform

## 목표
- 키 입력 처리 및 키바인딩 매핑
- 기본 키바인딩 (Cmd+T, Cmd+W 등)
- 커스터마이징 지원 (TOML)

## 완료 기준
- [ ] `Keybinding` 구조체: modifier, key, action
- [ ] `KeyHandler` 구현: 입력 이벤트 → action 매핑
- [ ] 기본 키바인딩 정의:
  - Cmd+T: 새 pane 가로 분할
  - Cmd+Shift+T: 새 pane 세로 분할
  - Cmd+W: pane 닫기
  - Cmd+H/J/K/L: 포커스 이동 (vim-like)
  - Cmd+Enter: 모드 전환 (Focus ↔ Split)
- [ ] TOML 기반 커스터마이징
- [ ] 단위 테스트
- [ ] `cargo test` 통과

## 포함할 모듈
- handler: 키 입력 처리
- keymap: 키바인딩 매핑 테이블
- action: 액션 정의 enum

## 노트
- macOS 특화: Cmd 키만 사용 (Ctrl은 Shell 용도)
- Phase 2에서 플랫폼별 키바인딩 확장
## 완료: 2026-03-01
