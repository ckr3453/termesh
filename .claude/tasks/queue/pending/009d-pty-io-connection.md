# PTY 입출력 연결

- phase: 1
- size: M
- blocked_by: 009c-layout-rendering-integration

## 목표
- 키보드 입력 → 활성 PTY write, PTY output → Terminal feed 파이프라인 구축

## 완료 기준
- [ ] 키보드 입력이 활성 세션의 PTY로 전달
- [ ] PTY 출력이 해당 세션의 Terminal로 feed
- [ ] Terminal 출력 변경 시 해당 pane 영역 재렌더링
- [ ] 포커스 전환 시 입력 라우팅 즉시 변경
- [ ] 비동기 PTY reader 스레드 → 메인 이벤트 루프 통합
- [ ] 유닛 테스트 통과
