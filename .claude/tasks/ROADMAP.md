# Termesh 로드맵

## Phase 1: macOS 네이티브 터미널 + AI 관제탑 (MVP)

**목표**: "출근해서 termesh open sigma-ai 하면 세션 리스트에 에이전트가 뜨고, 뭘 하고 있는지 보인다"

### 범용 터미널 기초 (8 tasks)
- [x] Phase 초기화
- [ ] Cargo workspace 설정
- [ ] termesh-core: 설정 구조체, 이벤트, 에러 정의
- [ ] termesh-pty: portable-pty PTY spawn/관리
- [ ] termesh-terminal: alacritty_terminal 래핑
- [ ] termesh-renderer: wgpu GPU 렌더링 파이프라인
- [ ] termesh-platform: macOS 네이티브 윈도우 (NSApplication/NSView)
- [ ] termesh-layout: pane 분할 엔진 (quad/dual)

### 입력 처리 & 범용 터미널 완성 (2 tasks)
- [ ] termesh-input: 키바인딩 엔진
- [ ] 기본 터미널 통합 테스트 (iterm2 대체 가능 수준)

### AI 에이전트 관제탑 (5 tasks)
- [ ] termesh-agent: 에이전트 어댑터, Claude Code 상태 추론
- [ ] termesh-diff: 파일 감시 (notify), diff 생성 (similar)
- [ ] 워크스페이스 프리셋 (TOML 설정 → 한 번에 실행)
- [ ] UI 레이아웃: Focus 모드 (좌측 세션 리스트 + 우측 풀사이즈 터미널 + 사이드 패널)
- [ ] UI 레이아웃: Split 모드 (2~4개 세션 동시 표시)

### 에이전트 상태 표시 & 최적화 (2 tasks)
- [ ] 세션 리스트에 에이전트 상태 아이콘/스피너 표시
- [ ] 성능 최적화 및 메모리 프로파일링

### 앱 통합 (1 task)
- [ ] termesh-app: 모든 크레이트 조립, 이벤트 루프, 데몬 시작

---

## Phase 2: 크로스 플랫폼 + 멀티 에이전트

**목표**: Linux/Windows 지원, 다양한 AI 도구 어댑터

---

## Phase 3: 원격 접속

**목표**: PC에서 나가도 폰으로 이어서 작업

---
