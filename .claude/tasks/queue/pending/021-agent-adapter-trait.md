# 에이전트 어댑터 trait 추상화

- phase: 2
- size: M

## 목표
- 현재 Claude Code 전용 로직을 범용 어댑터 trait으로 추상화
- 새 에이전트 추가 시 trait만 구현하면 되는 구조

## 완료 기준
- [ ] AgentAdapter trait 정의 (detect, parse_state, spawn 등)
- [ ] ClaudeCodeAdapter가 trait 구현
- [ ] 어댑터 레지스트리 (이름으로 어댑터 조회)
- [ ] 기존 테스트 전부 통과
- [ ] 어댑터 추가 가이드 문서 (doc comment)
