# termesh-app: 모든 크레이트 조립, 이벤트 루프, 데몬 시작

- phase: 1
- size: M
- blocked_by: 015-agent-status-display, 016-performance-optimization

## 목표
- Phase 1 MVP 완성
- 모든 크레이트를 하나의 통합 앱으로 조립
- 이벤트 루프 및 데몬 시작

## 완료 기준
- [ ] `main.rs`: 메인 진입점
- [ ] 크레이트 의존성 통합:
  - termesh-core (이벤트 버스)
  - termesh-pty (세션 관리)
  - termesh-terminal (에뮬레이션)
  - termesh-renderer (렌더링)
  - termesh-platform (macOS 윈도우)
  - termesh-layout (레이아웃)
  - termesh-input (키바인딩)
  - termesh-agent (상태 추론)
  - termesh-diff (파일 감시)
- [ ] 초기화 순서: Config → Platform → Renderer → Sessions → EventLoop
- [ ] 에러 처리 및 graceful shutdown
- [ ] 로깅: tracing-subscriber (RUST_LOG env)
- [ ] 런타임 설정 경로:
  - `~/.config/termesh/config.toml` (사용자)
  - `config/default.toml` (기본값)
- [ ] 커맨드라인 옵션:
  - `termesh open <workspace-name>`
  - `termesh` (기본 `zsh` 세션)
  - `--version`, `--help`
- [ ] 데몬 시작 (Unix Socket 수신)
- [ ] 통합 테스트 (E2E)
- [ ] 문서 작성: 설치, 사용법, 설정

## 구조
```
termesh-app/
├── main.rs             # 진입점
├── app.rs              # App 구조체, 이벤트 루프
├── config_loader.rs    # 설정 로드
├── command_handler.rs  # CLI 커맨드 처리
└── daemon.rs           # 데몬 모드
```

## 노트
- Phase 1 완성 후 릴리스 준비
