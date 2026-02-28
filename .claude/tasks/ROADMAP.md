# Termesh 로드맵

## Phase 1: macOS 네이티브 터미널 + AI 관제탑 (MVP)

**목표**: "출근해서 termesh open sigma-ai 하면 세션 리스트에 에이전트가 뜨고, 뭘 하고 있는지 보인다"

### 범용 터미널 기초 (8 tasks)
- [x] Phase 초기화
- [x] Cargo workspace 설정
- [x] termesh-core: 설정 구조체, 이벤트, 에러 정의
- [x] termesh-pty: portable-pty PTY spawn/관리
- [x] termesh-terminal: alacritty_terminal 래핑
- [x] termesh-renderer: wgpu GPU 렌더링 파이프라인
- [x] termesh-platform: 크로스플랫폼 윈도우 (winit)
- [x] termesh-layout: pane 분할 엔진 (quad/dual)

### 입력 처리 & 범용 터미널 완성 (2 tasks)
- [x] termesh-input: 키바인딩 엔진
- [x] 이벤트 루프 통합 + PTY 연결 (009a~009e)

### AI 에이전트 관제탑 (5 tasks)
- [x] termesh-agent: 에이전트 어댑터, Claude Code 상태 추론
- [x] termesh-diff: 파일 감시 (notify), diff 생성 (similar)
- [x] 워크스페이스 프리셋 (TOML 설정 → 한 번에 실행)
- [x] UI 레이아웃: Focus 모드 (좌측 세션 리스트 + 우측 풀사이즈 터미널 + 사이드 패널)
- [x] UI 레이아웃: Split 모드 (2~4개 세션 동시 표시)

### 에이전트 상태 표시 & 최적화 (2 tasks)
- [x] 세션 리스트에 에이전트 상태 아이콘/스피너 표시
- [x] 성능 최적화 및 메모리 프로파일링

### 앱 통합 (1 task)
- [x] termesh-app: 모든 크레이트 조립, CLI, 워크스페이스 로딩

---

## Phase 2: 크로스 플랫폼 + 멀티 에이전트 + DX 개선 + 인증

**목표**: Linux/Windows 지원, 다양한 AI 도구 어댑터, 개발 프로세스 고도화, 구독 인증

### 크로스 플랫폼 (3 tasks)
- [x] PTY Windows 호환 (ConPTY 분기, 경로 처리)
- [x] wgpu 백엔드 분기 (Vulkan/DX12/Metal, 소프트웨어 폴백)
- [x] CI/CD 멀티 플랫폼 빌드 파이프라인

### 멀티 에이전트 (3 tasks)
- [x] 에이전트 어댑터 trait 추상화 + AdapterRegistry
- [x] Gemini CLI 어댑터
- [x] Codex CLI 어댑터

### 구독 인증 (3 tasks)
- [x] 인증 API 클라이언트 (JWT, reqwest)
- [x] 로컬 라이선스 토큰 저장소
- [x] 앱 시작 인증 게이트 (오프라인 72시간 grace period)

### 개발 프로세스 (DX) (2 tasks)
- [x] `.claude/rules/` 자동 로드 규칙 분리
- [x] Hooks 파일 보호 (`.env`, 바이너리, `Cargo.lock` 직접 수정 감지)

---

## Phase 3: 원격 접속

**목표**: PC에서 나가도 폰으로 이어서 작업

---
