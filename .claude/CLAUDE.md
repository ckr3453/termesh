# Termesh - AI 에이전트 관제탑 + 범용 터미널

## 프로젝트 목적

**한 문장**: 여러 AI 코딩 에이전트를 하나의 워크스페이스에서 오케스트레이션하고, 일상 터미널로도 쓸 수 있는 도구.

**해결하는 문제**: 매일 여러 개의 Claude Code 프로세스를 분할 화면으로 열어 병렬 작업을 하는데, 각 에이전트의 상태를 파악하고 코드 변경을 추적하기 어려움.

**핵심 경험**: 하나의 터미널을 열면 모든 게 준비되고, 에이전트들의 상태가 시각화되며, 코드 변경이 실시간 diff로 보인다.

---

## 기술 스택

### 언어 & 런타임
- **Rust** (1.70+)
- **Tokio** (비동기 런타임)

### 핵심 라이브러리
| 용도 | 크레이트 |
|------|---------|
| 터미널 에뮬레이션 | `alacritty_terminal` |
| GPU 렌더링 | `wgpu` |
| PTY 관리 | `portable-pty` |
| 파일 감시 | `notify` |
| diff 생성 | `similar` |
| TOML 파싱 | `toml` + `serde` |
| macOS UI | `cocoa`, `core-foundation`, `objc2` |

### 아키텍처 레퍼런스
- **Tide**: Rust + wgpu + alacritty_terminal 조합의 터미널 워크스페이스 (크레이트 분리, GPU 파이프라인 참고)
- **tmux**: 세션 멀티플렉싱, 디태시 개념 참고
- **Codex/Cowork**: diff 뷰, 프리뷰 UX 참고

---

## 규칙 (Rules)

상세 규칙은 `.claude/rules/` 디렉토리에서 자동 로드됩니다:

- **`.claude/rules/rust-conventions.md`** — 코드 컨벤션 (네이밍, 에러 처리, 테스트, 문서)
- **`.claude/rules/git-conventions.md`** — Git 커밋 메시지 형식
- **`.claude/rules/security.md`** — 보안 규칙 (입력 검증, 비밀 관리)

---

## 아키텍처 개요

### 크레이트 구조
```
termesh/
├── crates/
│   ├── termesh-core/       # 공유 타입, 설정, 에러, 이벤트 버스
│   ├── termesh-pty/        # PTY spawn/관리, 세션 라이프사이클
│   ├── termesh-terminal/   # 터미널 에뮬레이션 (alacritty_terminal 래핑)
│   ├── termesh-renderer/   # wgpu GPU 렌더링
│   ├── termesh-layout/     # pane 분할, 탭, 레이아웃
│   ├── termesh-input/      # 키바인딩, 입력 처리
│   ├── termesh-agent/      # 에이전트 어댑터, 상태 추론
│   ├── termesh-diff/       # 파일 변경 감시, diff 생성
│   ├── termesh-platform/   # 플랫폼별 네이티브 레이어
│   └── termesh-app/        # 앱 진입점, 이벤트 루프
└── config/
    └── default.toml
```

### 통신 (Phase 1)
- **Unix Domain Socket** (`~/.termesh/termesh.sock`)
- 파일 퍼미션 `700` (본인만 접근)
- TCP 포트 노출 없음

### Phase 1 완성 기준
1. macOS 네이티브 터미널로 동작 (iterm2 대체 가능)
2. 세션 리스트에 에이전트 상태 표시
3. Code diff 사이드 패널 표시
4. Focus 모드 & Split 모드 토글 가능

---

## 개발 환경

### 설치
```bash
rustup default stable
brew install wgpu-cli  # GPU 디버깅 (optional)
```

### 빌드
```bash
cargo build --release
```

### 테스트
```bash
cargo test --all
```

### 실행 (Phase 1 중)
```bash
cargo run --bin termesh-app
```

---

## 비즈니스 모델

- **라이선스 인증**: 구독 기반 — Termesh 앱 사용 자체에 구독 인증 필요
- **AI 에이전트**: 사용자가 직접 설치한 CLI(Claude Code 등)를 그대로 사용 — Termesh가 API Key를 관리하지 않음

---

## 참고

- 로드맵: `.claude/tasks/ROADMAP.md`
- 태스크 추적: `.claude/tasks/queue/`
- Phase별 기록: `.claude/tasks/archive/`
