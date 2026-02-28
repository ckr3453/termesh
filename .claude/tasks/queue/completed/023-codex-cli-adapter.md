# Codex CLI 어댑터 구현

- phase: 2
- size: M
- blocked_by: 021-agent-adapter-trait

## 목표
- OpenAI Codex CLI의 출력 패턴 분석 및 상태 추론 어댑터 구현

## 완료 기준
- [ ] Codex CLI 출력 패턴 분석 및 정규식 정의
- [ ] CodexAdapter가 AgentAdapter trait 구현
- [ ] 상태 추론 (Idle, Thinking, Writing, Error 등)
- [ ] 워크스페이스 프리셋에서 codex 커맨드 인식
- [ ] 유닛 테스트 통과
