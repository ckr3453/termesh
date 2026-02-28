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

## 코드 컨벤션

### 파일 & 모듈
- 파일명: `snake_case`
- 모듈: pub 최소화, 필요한 것만 노출
- 한 파일 최대 300줄 원칙

### 네이밍
- **타입**: `PascalCase`
- **함수/변수**: `snake_case`
- **상수**: `SCREAMING_SNAKE_CASE`
- **에러 타입**: `<Action>Error` (e.g., `PtySpawnError`)

### 에러 처리
```rust
// ❌ 금지
fn load_config() -> Result<Config> {
    Ok(parse_toml()?) // 컨텍스트 없음
}

// ✅ 권장
fn load_config() -> Result<Config, ConfigError> {
    parse_toml()
        .context("Failed to parse config.toml")?
}
```

- **`anyhow`** 금지, **`thiserror`** 사용
- 모든 에러는 컨텍스트 메시지 포함
- 공개 API는 커스텀 에러 타입 정의

### 테스트
- 단위 테스트: 모듈 하단 `#[cfg(test)]`
- 통합 테스트: `tests/` 디렉토리
- 테스트명: `test_<function>_<scenario>` (e.g., `test_spawn_pty_with_invalid_cwd`)

### 코드 리뷰
- 태스크 완료 후 커밋 전, `code-reviewer` 서브에이전트로 변경된 파일 리뷰 실행
- 리뷰 관점: 보안 취약점, 에러 처리 누락, pub API 설계, 불필요한 복잡도
- 리뷰에서 발견된 이슈는 같은 커밋에서 수정 (별도 태스크 불필요)

### 문서
- pub 함수/타입은 반드시 doc 주석 포함
- 복잡한 로직은 인라인 주석으로 설명
- 공개 모듈: `//! Module description`

---

## Git 메시지 컨벤션

### 형식
```
<type>(<scope>): <subject>

<body>

Closes #<issue> (있으면)
Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>
```

### Type
- **feat**: 새 기능
- **fix**: 버그 수정
- **refactor**: 구조 개선 (기능 변화 없음)
- **perf**: 성능 개선
- **test**: 테스트 추가/수정
- **docs**: 문서만 변경
- **chore**: 의존성, 빌드 설정 등

### Scope
크레이트 또는 모듈 (e.g., `feat(pty): spawn PTY with working directory`)

### 예시
```
feat(layout): implement quad pane split

- Divide screen into 4 equal panes
- Add pane selection & focus with Cmd+Arrow
- Save/restore layout state

Closes #42
Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>
```

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

## 참고

- 로드맵: `.claude/tasks/ROADMAP.md`
- 태스크 추적: `.claude/tasks/queue/`
- Phase별 기록: `.claude/tasks/archive/`
