# 다중 세션 관리

- phase: 1
- size: M
- blocked_by: 009a-event-loop-app-integration

## 목표
- SessionId별로 Session + Terminal 인스턴스를 매핑 관리
- 세션 생성/삭제/전환 로직

## 완료 기준
- [ ] SessionManager 구조체 (HashMap<SessionId, (Session, Terminal)>)
- [ ] 세션 생성 시 PTY spawn + Terminal 인스턴스 생성
- [ ] 세션 삭제 시 PTY kill + 리소스 정리
- [ ] 활성 세션 전환
- [ ] App에서 SessionManager 통합
- [ ] 유닛 테스트 통과
