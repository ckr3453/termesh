# termesh-agent: 에이전트 어댑터, Claude Code 상태 추론

- phase: 1
- size: M
- blocked_by: 002-termesh-core

## 목표
- Claude Code 출력 스트림 분석 및 상태 추론
- 에이전트 어댑터 아키텍처 구축

## 완료 기준
- [ ] `AgentAdapter` trait 정의
- [ ] `ClaudeCodeAdapter` 구현 (Claude Code용 어댑터)
- [ ] 상태 패턴 정의 (정규식):
  - Thinking: "⏳|Thinking|Analyzing"
  - WritingCode: "Writing to |Creating |Updating "
  - RunningCommand: "Running: |Executing: |\\$ "
  - WaitingForInput: "Would you like|Do you want|y/n"
  - Error: "Error:|Failed:|✗"
  - Success: "✓|Done|Complete"
- [ ] 워크스페이스 프리셋 파싱 (TOML)
- [ ] 에이전트 상태 업데이트 이벤트 발행
- [ ] 단위 테스트
- [ ] `cargo test` 통과

## 포함할 모듈
- adapter: AgentAdapter trait
- claude_code: ClaudeCodeAdapter 구현
- preset: 워크스페이스 프리셋 파싱

## 노트
- Phase 2에서 Gemini, Codex 어댑터 추가
- 상태 패턴은 커스터마이징 가능하게
## 완료: 2026-03-01
