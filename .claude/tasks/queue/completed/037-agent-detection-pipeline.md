# PTY → 에이전트 상태 감지 파이프라인

- phase: 5
- size: M

## 목표
- PTY 출력을 AdapterRegistry에 통과시켜 에이전트 상태를 실시간 업데이트

## 완료 기준
- [ ] SessionManager.process_events()에서 PTY 데이터를 adapter.analyze_output()에 전달
- [ ] AdapterRegistry를 앱에서 인스턴스화하여 세션별 어댑터 매칭
- [ ] 에이전트 상태 변경 시 세션의 AgentState가 업데이트됨
- [ ] 상태 변화가 세션 리스트 UI에 반영 가능한 형태로 노출됨
