# 워크스페이스 프리셋 (TOML 설정 → 한 번에 실행)

- phase: 1
- size: S
- blocked_by: 010-termesh-agent

## 목표
- TOML 기반 워크스페이스 프리셋 저장/로드
- 한 번의 명령으로 모든 세션 자동 시작

## 완료 기준
- [ ] `Workspace` 구조체: name, default_mode, panes, side_panel
- [ ] TOML 파싱: `~/.config/termesh/workspaces/*.toml`
- [ ] `termesh open <workspace-name>` 커맨드
- [ ] 프리셋 파일 예제 작성: `config/examples/sigma-ai.toml`
- [ ] 설정 로드 에러 처리
- [ ] 단위 테스트
- [ ] `cargo test` 통과

## 포함할 모듈
- workspace: Workspace 구조체
- loader: TOML 파싱 및 로드

## 예제 프리셋
```toml
[workspace]
name = "sigma-ai"
default_mode = "focus"

[[workspace.panes]]
name = "backend"
agent = "claude"
cwd = "~/projects/sigma-ai/backend"
command = "claude"
role = "백엔드 API 개발"
```

## 노트
- 프리셋은 `~/.config/termesh/` 또는 프로젝트 루트 `.termesh.toml`에서 읽음
