# termesh-pty: PTY spawn/관리, 세션 라이프사이클

- phase: 1
- size: M
- blocked_by: 002-termesh-core

## 목표
- portable-pty를 래핑한 고수준 PTY 관리자
- 세션 생성, 실행, 종료, 재시작 기능

## 완료 기준
- [ ] `Session` 구조체: PTY 래퍼
- [ ] `spawn_session(cwd, command, args)` 함수
- [ ] `kill_session()`, `restart_session()` 구현
- [ ] 입출력 스트림 처리 (tokio channel)
- [ ] 신호 처리 (SIGTERM, SIGKILL)
- [ ] 단위 테스트
- [ ] `cargo test` 통과

## 포함할 모듈
- session: Session 구조체 및 생명주기 관리
- pty: portable-pty 래퍼
- reader: 비동기 입출력 스트림

## 노트
- Windows 지원은 Phase 2에서 (이번엔 macOS만)
- PTY spawn 실패 시 에러 로깅
