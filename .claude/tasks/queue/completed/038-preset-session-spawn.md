# 워크스페이스 프리셋 → 실제 세션 스폰

- phase: 5
- size: M

## 목표
- `termesh open sigma-ai` 실행 시 프리셋에 정의된 세션들을 실제 PTY로 생성

## 완료 기준
- [ ] App::from_preset()에서 프리셋의 각 세션을 SessionManager.spawn()으로 생성
- [ ] 프리셋의 command, args, cwd가 SessionConfig에 매핑됨
- [ ] 프리셋의 agent 타입이 세션에 연결됨
- [ ] 프리셋의 레이아웃(panes)이 SplitLayout에 반영됨
- [ ] spawn_default_shell() 대신 프리셋 세션들이 생성됨
