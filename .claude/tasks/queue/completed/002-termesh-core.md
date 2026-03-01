# termesh-core: 설정 구조체, 이벤트, 에러 정의

- phase: 1
- size: M
- blocked_by: 001-cargo-workspace

## 목표
- 모든 크레이트가 의존하는 공유 타입 정의
- TOML 기반 설정 시스템 구축
- 이벤트 버스 아키텍처 설계

## 완료 기준
- [ ] `Config` 구조체: terminal, keybindings, daemon 섹션
- [ ] `Error` enum with thiserror
- [ ] `Event` enum: SessionCreated, SessionClosed, FileChanged, AgentStateChanged
- [ ] `EventBus` pub/sub 구현
- [ ] 단위 테스트 작성
- [ ] `cargo test` 통과

## 포함할 모듈
- config: TOML 파싱, 기본값
- error: 커스텀 에러 타입
- event: 이벤트 정의 및 버스
- types: Session, Pane, AgentState 등 기본 타입

## 노트
- serde + toml 사용
- Unix Domain Socket 경로 정의: `~/.termesh/termesh.sock`
